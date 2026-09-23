pub mod download_url;
pub mod downloader;
pub mod extractor;
pub mod progress;

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::PathBuf;
use util::consts::SDKM_TMP_DIR;
use util::path::{get_installed_sdks_dir, get_sdkm_home};
use util::sdk::{BuiltinSdk, Sdk};
use util::terminal::prompt_confirm;
use util::{bail_bug, detail, info, try_bug, warning};

use crate::manager::SdkManager;
use crate::version::github_releases::{ensure_per_page, github_api_headers, is_github_releases_url};
use crate::version::{
    ResolvedVersion, VersionSource, get_version_discovery, resolve_java_version, resolve_sdk_version,
};
use download_url::build_download_url;
use downloader::{build_reqwest_client, download_with_retry};
use extractor::{extract_archive, normalize_extracted_dir, verify_extraction};
use progress::InstallProgress;
use util::builtin::find_seed_by_variant;

impl SdkManager {
    /// 安装 SDK 版本（同步入口，内部创建 tokio runtime 驱动异步流程）
    pub fn install_sdk(&mut self, sdk: &Sdk, version_input: &str, auto_switch: bool) -> Result<()> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        // 用 ? 传播错误，不整体 try_bug!——用户取消、网络/版本解析失败等非 bug 错误不应触发 bug report 提示；
        // 真正的 bug（解压/校验失败等）已在 install_sdk_async 内部用 try_bug!/bail_bug! 精确标记 BugReportError
        rt.block_on(self.install_sdk_async(sdk, version_input, auto_switch))?;
        Ok(())
    }

    /// 异步安装核心流程：通用主备 + 模糊匹配 + 交互确认
    async fn install_sdk_async(&mut self, sdk: &Sdk, version_input: &str, auto_switch: bool) -> Result<()> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdk_name = sdk.to_string();

        // 先清理该 sdk 上次中断的残留，避免旧 sdk.zip/extracted 污染本次
        let sdk_tmp_root = get_sdkm_home()?.join(SDKM_TMP_DIR).join(&sdk_name);
        if sdk_tmp_root.exists()
            && let Err(e) = fs::remove_dir_all(&sdk_tmp_root)
        {
            warning!("failed to clean stale tmp at {}: {}", sdk_tmp_root.display(), e);
        }

        // ── Phase 1: 版本解析 ────────────────────────────────────
        let resolve_pb = InstallProgress::new_resolve(&sdk_name, version_input);
        let client = build_reqwest_client(&self.config.network)?;

        let resolved = if let Sdk::Built(BuiltinSdk::Java) = sdk {
            // Java 有两步查询逻辑（available_releases → assets API），独立处理
            resolve_java_version(&client, version_input).await?
        } else {
            // 通用流程：版本源统一读 config（内置 SDK 的种子已由 ensure_builtin_sdks 物化，
            // 用户可 config set sdk.<name>.version_url 换源/镜像）+ 缓存兜底 + 模糊匹配
            let version_url = sdk_conf.version_url.clone().unwrap_or_default();
            let version_fallback_url = sdk_conf.version_fallback_url.clone();

            if version_url.is_empty() && version_fallback_url.as_ref().is_none_or(|s| s.is_empty()) {
                // 无版本发现源 → 仅支持精确版本
                if !version_input.contains('.') {
                    bail!(
                        "exact version required, fuzzy '{}' not supported (no version_url configured)",
                        version_input
                    );
                }
                let fv = version_input.split('.').next().map(|v| v.to_string());
                ResolvedVersion {
                    full_version: version_input.to_string(),
                    feature_version: fv,
                    release_tag: None,
                    download_url: None,
                    fuzzy_matched: false,
                }
            } else {
                // GH 版本源自动补 per_page=100（GH 默认 30 条，高频发版工具不够）
                let version_url = ensure_per_page(&version_url);
                let cache_key = sdk_name.to_lowercase();
                // GitHub API 请求统一注入 Accept header（主源或备源任一为 GH API 即注入，
                // 覆盖 Python 的 GH 备源与全部标准 B SDK）
                let headers = if is_github_releases_url(&version_url)
                    || version_fallback_url.as_ref().is_some_and(|u| is_github_releases_url(u))
                {
                    Some(github_api_headers())
                } else {
                    None
                };
                let source = VersionSource {
                    primary_url: version_url.clone(),
                    secondary_url: version_fallback_url,
                };
                let discovery = get_version_discovery(sdk, &version_url);
                resolve_sdk_version(
                    &client,
                    discovery.as_ref(),
                    &source,
                    &cache_key,
                    &sdk_name,
                    version_input,
                    headers,
                    self.config.network.cache_ttl_secs as u64,
                )
                .await?
            }
        };

        // resolve 已完成,先停止 spinner,避免其 steady_tick 的 \r 覆盖后续交互提示
        resolve_pb.finish_with_message(format!("✅ Resolved: {} → {}", version_input, resolved.full_version));

        // ── 模糊匹配交互确认 ────────────────────────────────────
        if resolved.fuzzy_matched {
            let confirmed = prompt_confirm(&format!(
                "Resolved '{}' → '{}'. Install this version?",
                version_input, resolved.full_version
            ))?;
            if !confirmed {
                bail!("Installation cancelled by user");
            }
        }

        let full_version = &resolved.full_version;

        // ── Phase 2: 检查本地是否已安装 ──────────────────────────
        let store_dir = get_installed_sdks_dir()?;
        let version_dir = store_dir.join(&sdk_name).join(full_version);
        if version_dir.exists() {
            warning!("{} {} is already installed locally.", sdk_name, full_version);
            info!("If you want to reinstall, remove it first: delete {}", version_dir.display());
            if auto_switch {
                self.switch_sdk_to_version(sdk, full_version)?;
            }
            return Ok(());
        }
        info!("{} {} is not installed locally, proceeding...", sdk_name, full_version);

        // ── Phase 3: 构建下载 URL（主/备）─────────────────────────
        let download_url = if let Some(direct) = resolved.download_url.clone() {
            // 版本发现源直接给出下载直链（如部分 GitHub release）
            direct
        } else {
            // 走 download_url 模板渲染；模板缺失 = 本地 switch-only SDK，无法远程安装
            let template = sdk_conf.download_url.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "`{}` has no download_url configured and cannot be installed remotely. \
                     This is a local switch-only SDK — place the version dir at `store/{}/{}` manually, \
                     or set a download_url via `sdkm config set sdk.{}.download_url <url>`.",
                    sdk_name,
                    sdk_name,
                    full_version,
                    sdk_name
                )
            })?;
            build_download_url(sdk, sdk_conf, template, &resolved)?
        };
        detail!("Download URL (primary): {}", download_url);

        let download_fallback_url = sdk_conf
            .download_fallback_url
            .clone()
            .or_else(|| {
                // 内置 SDK 的静态备源
                match sdk {
                    Sdk::Built(b) => {
                        find_seed_by_variant(b).and_then(|c| c.download_fallback_url.map(|s| s.to_string()))
                    }
                    _ => None,
                }
            })
            .map(|fallback_template| {
                // 备源 URL 模板一律渲染（含直链场景：{version} 取解析后的版本号，
                // 支持国内 gh-proxy 类镜像模板作为 GH 直链失败的备源）
                build_download_url(sdk, sdk_conf, &fallback_template, &resolved).unwrap_or(fallback_template)
            });

        // ── Phase 4: 创建临时目录 ─────────────────────────────────
        let sdkm_home = get_sdkm_home()?;
        let tmp_root = sdkm_home.join(SDKM_TMP_DIR);
        let tmp_dir = tmp_root.join(&sdk_name).join(full_version);
        try_bug!(fs::create_dir_all(&tmp_dir).context("Failed to create temporary directory"));
        detail!("Temporary directory: {}", tmp_dir.display());

        let archive_filename = derive_archive_filename(&download_url);
        let tmp_archive_path = tmp_dir.join(&archive_filename);
        let tmp_extracted_path = tmp_dir.join("extracted");
        let is_zip = archive_filename.ends_with(".zip");

        // ── Phase 5: 下载（主/备）───────────────────────────────────
        let download_client = build_reqwest_client(&self.config.network)?;
        let download_pb = InstallProgress::new_download(&sdk_name, full_version);

        // 先尝试主源下载
        let download_result =
            download_with_retry(&download_client, &download_url, &tmp_archive_path, &download_pb.pb, 3).await;

        let final_download_url = if download_result.is_err() && download_fallback_url.is_some() {
            // 主源下载失败，尝试备源
            let fallback_url = download_fallback_url.unwrap();
            warning!("Primary download failed, trying fallback URL: {}", truncate(&fallback_url, 80));
            let fallback_pb = InstallProgress::new_download(&sdk_name, full_version);
            let fallback_filename = derive_archive_filename(&fallback_url);
            let fallback_archive_path = tmp_dir.join(&fallback_filename);
            download_with_retry(&download_client, &fallback_url, &fallback_archive_path, &fallback_pb.pb, 3).await?;
            // 更新后续使用的文件名和路径
            fallback_url
        } else {
            download_result?;
            download_url
        };
        download_pb.finish_with_message(format!("✅ Downloaded {} {}", sdk_name, full_version));

        // ── Phase 6: 解压 ────────────────────────────────────────
        let actual_archive_path = tmp_dir.join(derive_archive_filename(&final_download_url));
        let extract_pb = if is_zip {
            InstallProgress::new_extract_zip(&sdk_name, full_version)
        } else {
            InstallProgress::new_extract_tar_gz(&sdk_name, full_version)
        };
        try_bug!(extract_archive(&actual_archive_path, &tmp_extracted_path, &extract_pb.pb));
        extract_pb.finish_with_message(format!("✅ Extracted {} {}", sdk_name, full_version));

        // ── Phase 7: 验证解压结果 ────────────────────────────────
        if !tmp_extracted_path.exists() {
            cleanup_temp(&tmp_dir)?;
            bail_bug!("Extraction failed: no output directory found");
        }

        // ── Phase 8: 目录调整 ────────────────────────────────────
        let move_pb = InstallProgress::new_verify();
        try_bug!(normalize_extracted_dir(&tmp_extracted_path, &version_dir));
        move_pb.finish_with_message(format!("📂 Moved to {}", version_dir.display()));

        // ── Phase 9: 验证安装 ────────────────────────────────────
        let verify_pb = InstallProgress::new_verify();
        if let Err(e) = verify_extraction(&version_dir, sdk_conf.bin_dir.as_deref()) {
            let _ = fs::remove_dir_all(&version_dir);
            cleanup_temp(&tmp_dir)?;
            bail_bug!("Installation verification failed: {}. Rolled back.", e);
        }
        verify_pb.finish_with_message(format!("✅ Verified {} {}", sdk_name, full_version));

        // ── Phase 10: 清理临时文件 ──────────────────────────────
        cleanup_temp(&tmp_dir)?;
        detail!("Temporary files cleaned up.");

        // ── Phase 11: 自动切换 ──────────────────────────────────
        if auto_switch {
            InstallProgress::print_switching(&sdk_name, full_version);
            self.switch_sdk_to_version(sdk, full_version)?;
        }

        // ── Phase 12: 安装完成 ──────────────────────────────────
        InstallProgress::print_success(&sdk_name, full_version, auto_switch);

        Ok(())
    }
}

/// 清理临时目录
fn cleanup_temp(tmp_dir: &PathBuf) -> Result<()> {
    if tmp_dir.exists() {
        fs::remove_dir_all(tmp_dir).context("Failed to clean up temporary directory")?;
    }
    if let Some(parent) = tmp_dir.parent()
        && parent.exists()
    {
        let entries = fs::read_dir(parent);
        if let Ok(entries) = entries
            && entries.count() == 0
        {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(())
}

/// 从下载 URL 推导压缩包文件名
fn derive_archive_filename(url: &str) -> String {
    let path_part = url.split('?').next().unwrap_or(url);
    let filename = path_part.rsplit('/').next().unwrap_or("download");

    if filename.contains(".") {
        filename.replace("%2B", "+")
    } else if cfg!(target_os = "windows") {
        "sdk.zip".to_string()
    } else {
        "sdk.tar.gz".to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}
