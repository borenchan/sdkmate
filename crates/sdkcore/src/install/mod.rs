pub mod download_url;
pub mod downloader;
pub mod extractor;

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::PathBuf;
use util::consts::SDKM_TMP_DIR;
use util::path::{get_installed_sdks_dir, get_sdkm_home};
use util::sdk::{BuiltinSdk, Sdk};
use util::{bail_bug, try_bug};

use crate::manager::SdkManager;
use crate::report::ProgressKind;
use crate::version::github_releases::{ensure_per_page, github_api_headers, is_github_releases_url};
use crate::version::{
    ResolvedVersion, VersionSource, get_version_discovery, resolve_java_version, resolve_sdk_version,
};
use download_url::build_download_url;
use downloader::{build_download_client, build_reqwest_client, download_with_retry};
use extractor::{extract_archive, normalize_extracted_dir, verify_extraction};
use util::builtin::find_seed_by_variant;

impl SdkManager {
    /// 解析安装版本（模糊输入 → 精确版本；网络查询/缓存/主备源）。只读零副作用，
    /// 供调用端展示解析结果并获取用户确认后，再经 [`install_sdk`] 执行安装；
    /// 模糊确认在调用端做（core 零 stdin）
    pub fn resolve_install_version(&self, sdk: &Sdk, version_input: &str) -> Result<ResolvedVersion> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        rt.block_on(self.resolve_install_version_async(sdk, version_input))
    }

    /// 异步版本解析（原 install 流程 Phase 1 段整体迁入）
    async fn resolve_install_version_async(&self, sdk: &Sdk, version_input: &str) -> Result<ResolvedVersion> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdk_name = sdk.to_string();

        // 文案区分：ls 浏览模式无版本输入（空串）→ 拉取远程列表；install 场景为版本解析
        let resolve_title = if version_input.is_empty() {
            format!("Fetching remote versions for {}...", sdk_name)
        } else {
            format!("Resolving version '{}' for {}...", version_input, sdk_name)
        };
        let resolve_pb = self.sink.progress(ProgressKind::Resolve, resolve_title);
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
                    &self.config.network,
                    self.sink.as_ref(),
                )
                .await?
            }
        };

        // resolve 已完成,先停止 spinner（finish 定格帧也供非 TTY 之外的展示收尾）
        resolve_pb.finish_with_message(format!("✅ Resolved: {} → {}", version_input, resolved.full_version));
        Ok(resolved)
    }

    /// 安装已解析的 SDK 版本（同步入口，内部创建 tokio runtime 驱动异步流程）。
    /// 版本解析与模糊确认归调用端（resolve_install_version）；执行段保留已装检测自查
    pub fn install_sdk(&mut self, sdk: &Sdk, resolved: &ResolvedVersion, auto_switch: bool) -> Result<()> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        // 用 ? 传播错误，不整体 try_bug!——用户取消、网络/版本解析失败等非 bug 错误不应触发 bug report 提示；
        // 真正的 bug（解压/校验失败等）已在 install_sdk_async 内部用 try_bug!/bail_bug! 精确标记 BugReportError
        rt.block_on(self.install_sdk_async(sdk, resolved, auto_switch))?;
        Ok(())
    }

    /// 异步安装核心流程：通用主备 + 已装检测 + 下载解压验证清理
    async fn install_sdk_async(&mut self, sdk: &Sdk, resolved: &ResolvedVersion, auto_switch: bool) -> Result<()> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdk_name = sdk.to_string();

        // 先清理该 sdk 上次中断的残留，避免旧 sdk.zip/extracted 污染本次
        let sdk_tmp_root = get_sdkm_home()?.join(SDKM_TMP_DIR).join(&sdk_name);
        if sdk_tmp_root.exists()
            && let Err(e) = fs::remove_dir_all(&sdk_tmp_root)
        {
            self.sink
                .warning(format!("failed to clean stale tmp at {}: {}", sdk_tmp_root.display(), e));
        }

        let full_version = &resolved.full_version;

        // ── Phase 2: 检查本地是否已安装 ──────────────────────────
        let store_dir = get_installed_sdks_dir()?;
        let version_dir = store_dir.join(&sdk_name).join(full_version);
        if version_dir.exists() {
            self.sink
                .warning(format!("{} {} is already installed locally.", sdk_name, full_version));
            self.sink.info(format!(
                "If you want to reinstall, remove it first: delete {}",
                version_dir.display()
            ));
            if auto_switch {
                self.switch_sdk_to_version(sdk, full_version)?;
            }
            return Ok(());
        }
        self.sink
            .info(format!("{} {} is not installed locally, proceeding...", sdk_name, full_version));

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
        // GitHub 下载镜像重写（白名单域名拼前缀；关闭态原样返回）
        let download_url = self.config.network.mirror_github_url(&download_url);
        self.sink.detail(format!("Download URL (primary): {}", download_url));

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
            })
            // 备源同样过镜像层（白名单内重写，与主源规则一致）
            .map(|fallback_url| self.config.network.mirror_github_url(&fallback_url));

        // ── Phase 4: 创建临时目录 ─────────────────────────────────
        let sdkm_home = get_sdkm_home()?;
        let tmp_root = sdkm_home.join(SDKM_TMP_DIR);
        let tmp_dir = tmp_root.join(&sdk_name).join(full_version);
        try_bug!(fs::create_dir_all(&tmp_dir).context("Failed to create temporary directory"));
        self.sink.detail(format!("Temporary directory: {}", tmp_dir.display()));

        let archive_filename = derive_archive_filename(&download_url);
        let tmp_archive_path = tmp_dir.join(&archive_filename);
        let tmp_extracted_path = tmp_dir.join("extracted");
        let is_zip = archive_filename.ends_with(".zip");

        // ── Phase 5: 下载（主/备）───────────────────────────────────
        let download_client = build_download_client(&self.config.network)?;
        let download_pb = self
            .sink
            .progress(ProgressKind::Download, format!("Downloading {} {}...", sdk_name, full_version));

        // 先尝试主源下载
        let download_result = download_with_retry(
            &download_client,
            &download_url,
            &tmp_archive_path,
            download_pb.as_ref(),
            self.sink.as_ref(),
            3,
        )
        .await;

        // 主备源任一成功的 bar（成功消息由实际下载的 bar 收尾，主源 bar 不复活）
        let active_pb;
        let final_download_url = if download_result.is_err() && download_fallback_url.is_some() {
            // 主源下载失败，尝试备源
            let fallback_url = download_fallback_url.unwrap();
            self.sink.warning(format!(
                "Primary download failed, trying fallback URL: {}",
                truncate(&fallback_url, 80)
            ));
            // 主源 bar 显式收尾（停 tick + 清残帧）：句柄仍存活、Drop 兜底不触发，
            // 不收尾会与备源 bar 双活重绘互相踩踏（终端闪烁，既有缺陷，本次修复）
            download_pb.finish_and_clear();
            let fallback_pb = self
                .sink
                .progress(ProgressKind::Download, format!("Downloading {} {}...", sdk_name, full_version));
            let fallback_filename = derive_archive_filename(&fallback_url);
            let fallback_archive_path = tmp_dir.join(&fallback_filename);
            download_with_retry(
                &download_client,
                &fallback_url,
                &fallback_archive_path,
                fallback_pb.as_ref(),
                self.sink.as_ref(),
                3,
            )
            .await?;
            // 更新后续使用的文件名和路径
            active_pb = fallback_pb;
            fallback_url
        } else {
            download_result?;
            active_pb = download_pb;
            download_url
        };
        active_pb.finish_with_message(format!("✅ Downloaded {} {}", sdk_name, full_version));

        // ── Phase 6: 解压 ────────────────────────────────────────
        let actual_archive_path = tmp_dir.join(derive_archive_filename(&final_download_url));
        let extract_kind = if is_zip {
            ProgressKind::ExtractZip
        } else {
            ProgressKind::ExtractTarGz
        };
        let extract_pb = self
            .sink
            .progress(extract_kind, format!("Extracting {} {}...", sdk_name, full_version));
        try_bug!(extract_archive(&actual_archive_path, &tmp_extracted_path, extract_pb.as_ref()));
        extract_pb.finish_with_message(format!("✅ Extracted {} {}", sdk_name, full_version));

        // ── Phase 7: 验证解压结果 ────────────────────────────────
        if !tmp_extracted_path.exists() {
            cleanup_temp(&tmp_dir)?;
            bail_bug!("Extraction failed: no output directory found");
        }

        // ── Phase 8: 目录调整 ────────────────────────────────────
        let move_pb = self
            .sink
            .progress(ProgressKind::Verify, "Verifying installation...".to_string());
        try_bug!(normalize_extracted_dir(&tmp_extracted_path, &version_dir));
        move_pb.finish_with_message(format!("📂 Moved to {}", version_dir.display()));

        // ── Phase 9: 验证安装 ────────────────────────────────────
        let verify_pb = self
            .sink
            .progress(ProgressKind::Verify, "Verifying installation...".to_string());
        if let Err(e) = verify_extraction(&version_dir, sdk_conf.bin_dir.as_deref()) {
            let _ = fs::remove_dir_all(&version_dir);
            cleanup_temp(&tmp_dir)?;
            bail_bug!("Installation verification failed: {}. Rolled back.", e);
        }
        verify_pb.finish_with_message(format!("✅ Verified {} {}", sdk_name, full_version));

        // ── Phase 10: 清理临时文件 ──────────────────────────────
        cleanup_temp(&tmp_dir)?;
        self.sink.detail("Temporary files cleaned up.".to_string());

        // ── Phase 11: 自动切换 ──────────────────────────────────
        if auto_switch {
            self.sink.info(format!("🔗 Switching {} to {}...", sdk_name, full_version));
            self.switch_sdk_to_version(sdk, full_version)?;
        }

        // ── Phase 12: 安装完成 ──────────────────────────────────
        if auto_switch {
            self.sink
                .success(format!("{} {} installed and switched successfully!", sdk_name, full_version));
        } else {
            self.sink.success(format!(
                "{} {} installed successfully! (not switched, use 'sdkm switch {} {}' to activate)",
                sdk_name, full_version, sdk_name, full_version
            ));
        }

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
