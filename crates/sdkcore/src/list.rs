use crate::install::downloader::build_reqwest_client;
use crate::install::progress::InstallProgress;
use crate::manager::SdkManager;
use crate::version::{VersionSource, fetch_version_data, get_version_discovery};
use anyhow::{Context, Result, bail};
use crossterm::style::Stylize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;
use util::consts::STATUS_ACTIVE;
use util::path::{dir_size, format_bytes, get_installed_sdks_dir, get_sdkm_home};
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::find_builtin_sdk_config;
use util::terminal::{ColumnColor, pad_right, print_table};
use util::{divider, info, try_bug, warning};

// ─── 数据结构 ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct SdkVersionItem {
    pub sdk: Sdk,
    pub sdk_version: String,
    pub sdk_dir: PathBuf,
    pub is_active: bool,
    /// 版本目录占用大小（字节）
    pub size_bytes: u64,
}

impl SdkVersionItem {
    pub fn new(sdk: Sdk, sdk_dir: PathBuf, is_active: bool) -> Self {
        let cow = sdk_dir.file_name().unwrap_or(OsStr::new("(empty dir)")).to_string_lossy();
        let size_bytes = dir_size(&sdk_dir).unwrap_or(0);
        Self {
            sdk,
            sdk_version: cow.to_string(),
            sdk_dir,
            is_active,
            size_bytes,
        }
    }
}

/// 远程版本条目(附带本地安装状态)
#[derive(Debug, Clone)]
pub struct RemoteVersionItem {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub install_status: InstallStatus,
    /// 源 URL(用于透明展示)
    pub source_url: String,
}

/// 远程版本展示用的安装状态标记
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallStatus {
    /// 本地未安装
    NotInstalled,
    /// 已安装但非当前激活版本
    Installed,
    /// 已安装且为当前激活版本
    Active,
}

// ─── 本地列表 ────────────────────────────────────────────────────

/// 远程版本列表结果(含截断前的总数)
pub struct RemoteVersionResult {
    pub items: Vec<RemoteVersionItem>,
    /// 应用数量限制前的总数(用于 TUI 头部展示)
    pub total_count: usize,
}

impl SdkManager {
    /// 打印所有已安装 SDK 及其当前版本(非交互式摘要)
    ///
    /// 三层结构样式（标题/列头/条目颜色区分，与 `current` 的表格样式区分）：
    /// ```text
    /// ℹ️  installed sdks:
    ///        sdk     current  size
    /// ✅  1. java    8.0.492  518.3 MB
    /// ```
    pub fn show_local_sdk_list(&self) -> Result<()> {
        let sdk_dir = get_installed_sdks_dir()?;
        let mut items: Vec<(String, String, String)> = Vec::new();
        for entry in sdk_dir.read_dir()?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let sdk_name = entry.file_name().to_string_lossy().to_string();
            let current = self
                .match_valid_sdk(&sdk_name)
                .ok()
                .and_then(|sdk| self.config.find_sdk_ok(&sdk).ok())
                .and_then(|conf| conf.current_version.clone())
                .unwrap_or_else(|| "N/A".to_string());
            let size = format_bytes(dir_size(&path).unwrap_or(0));
            items.push((sdk_name, current, size));
        }
        divider!();
        if items.is_empty() {
            info!("no installed sdks. run `sdkm install <sdk> <version>` to get started.");
        } else {
            info!("installed sdks:");
            let headers = ["sdk", "current", "size"];
            let active_w = UnicodeWidthStr::width(STATUS_ACTIVE);
            // 数据行前缀 = `{status} {idx:>2}. `，表头行用同等宽度空格缩进对齐数据列
            let prefix_w = active_w + 1 + 2 + 2;
            // 列宽取表头与数据中的最大显示宽度
            let sdk_w = items
                .iter()
                .map(|(s, _, _)| s.as_str().width())
                .chain(std::iter::once(headers[0].width()))
                .max()
                .unwrap_or(0);
            let ver_w = items
                .iter()
                .map(|(_, v, _)| v.as_str().width())
                .chain(std::iter::once(headers[1].width()))
                .max()
                .unwrap_or(0);
            let size_w = items
                .iter()
                .map(|(_, _, z)| z.as_str().width())
                .chain(std::iter::once(headers[2].width()))
                .max()
                .unwrap_or(0);
            // 表头行（青粗体，缩进对齐数据列起始位置）
            let header_line = [
                pad_right(headers[0], sdk_w),
                pad_right(headers[1], ver_w),
                pad_right(headers[2], size_w),
            ]
            .join("  ");
            println!("{}{}", " ".repeat(prefix_w), header_line.cyan().bold());
            // 数据行：status + 序号 + sdk(白粗) + version(绿) + size(灰)
            for (i, (sdk, cur, size)) in items.iter().enumerate() {
                let status = if cur == "N/A" {
                    " ".repeat(active_w)
                } else {
                    STATUS_ACTIVE.to_string()
                };
                println!(
                    "{} {:>2}. {}  {}  {}",
                    status,
                    i + 1,
                    pad_right(sdk, sdk_w).bold(),
                    pad_right(cur, ver_w).green(),
                    size.as_str().dark_grey(),
                );
            }
        }
        divider!();
        Ok(())
    }

    /// 显示单个或全部 SDK 的当前版本(供 `sdkm current` 命令使用)
    ///
    /// 语义只需当前版本，不带 size 列。多 SDK 用表头表格样式（与 `ls` 的序号样式区分）；单 SDK 单行着色。
    pub fn show_local_sdks_current(&self, sdk: Option<Sdk>) -> Result<()> {
        if let Some(sdk) = sdk {
            let conf = self.config.find_sdk_ok(&sdk)?;
            let current = conf.current_version.clone().unwrap_or_else(|| "N/A".to_string());
            println!("ℹ️  {} {}", sdk.to_string().bold(), current.green());
            return Ok(());
        }
        info!("sdkm home: {}", get_sdkm_home()?.display());
        let mut rows: Vec<Vec<String>> = Vec::new();
        for entry in get_installed_sdks_dir()?.read_dir()?.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            // store 目录可能含未在 config 注册的子目录(如手动放入或遗留),跳过而非中断整体列表
            let sdk = match self.match_valid_sdk(&name) {
                Ok(s) => s,
                Err(e) => {
                    warning!("skip unregistered SDK directory `{}`: {}", name, e);
                    continue;
                }
            };
            let sdk_conf = self.config.find_sdk_ok(&sdk)?;
            let current = sdk_conf.current_version.clone().unwrap_or_else(|| "N/A".to_string());
            rows.push(vec![sdk.to_string(), current]);
        }
        divider!();
        if rows.is_empty() {
            info!("no active sdks.");
        } else {
            print_table(&["sdk", "current"], &rows, &[ColumnColor::Bold, ColumnColor::Green]);
        }
        divider!();
        Ok(())
    }

    /// 获取本地 SDK 版本列表(数据层,供 TUI 选择器使用)
    pub fn list_local_sdk_versions(&self, sdk: &Sdk) -> Result<Vec<SdkVersionItem>> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdks_root_dir = get_installed_sdks_dir()?;
        let sdk_dir = sdks_root_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy() == sdk.to_string());
        if let Some(sdk_dir) = sdk_dir {
            let result = sdk_dir
                .path()
                .read_dir()?
                .filter_map(|entry| entry.ok())
                .map(|sdk_version| {
                    let sdk_version_dir = sdk_version.path();
                    let is_active = sdk_conf
                        .current_version
                        .clone()
                        .is_some_and(|current| current == sdk_version.file_name().to_string_lossy().as_ref());
                    SdkVersionItem::new(sdk.clone(), sdk_version_dir, is_active)
                })
                .collect();
            return Ok(result);
        }
        info!("SDK '{}' not found in store. Please install a version first.", sdk);
        Ok(vec![])
    }

    /// 打印本地版本列表(带 ✅ 激活标记,非 TUI 的回退展示)
    pub fn show_local_sdk_version_list(&self, sdk: &Sdk) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let mut i = 1;
        let prefix = " ".repeat(UnicodeWidthStr::width(STATUS_ACTIVE));
        versions.iter().for_each(|v| {
            info!(
                "{} {:>2}. {}",
                if v.is_active { STATUS_ACTIVE } else { &prefix },
                i,
                v.sdk_version
            );
            i += 1;
        });
        Ok(())
    }

    // ─── 远程列表 ────────────────────────────────────────────────

    /// 异步:拉取远程版本——带 spinner 的核心逻辑
    ///
    /// 复用 version 模块的 fetch_version_data + parse_version_data 管线。
    /// 缓存优先 + TTL,确保后续调用即时响应。
    async fn fetch_remote_versions_async(&self, sdk: &Sdk, limit: u32) -> Result<RemoteVersionResult> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let strategy = get_version_discovery(sdk);
        let client = build_reqwest_client(&self.config.network)?;
        let sdk_name = sdk.to_string();
        let cache_key = sdk_name.to_lowercase();

        // Maven 无 version_url → 不支持远程版本列表
        if let Sdk::Built(BuiltinSdk::Maven) = sdk {
            bail!("Maven does not support remote version listing. Specify an exact version to install.");
        }

        // 构建版本源 URL(主/备 + 用于透明展示的源 URL)
        let (primary_url, secondary_url, source_display_url) = match sdk {
            Sdk::Built(b) => {
                let cfg = find_builtin_sdk_config(b).context(format!("no builtin config for {}", sdk_name))?;
                // 内置配置缺失属于程序 bug，标记 BugReportError
                (
                    cfg.version_url.to_string(),
                    cfg.version_fallback_url.map(|s| s.to_string()),
                    if !cfg.version_url.is_empty() {
                        cfg.version_url.to_string()
                    } else {
                        "N/A".to_string()
                    },
                )
            }
            Sdk::Custom(_) => {
                let url = sdk_conf.version_url.clone().unwrap_or_default();
                (
                    url.clone(),
                    sdk_conf.version_fallback_url.clone(),
                    if !url.is_empty() { url } else { "N/A".to_string() },
                )
            }
        };

        // 无版本发现源 → 无法列出远程版本
        if primary_url.is_empty() && secondary_url.as_ref().is_none_or(|s| s.is_empty()) {
            bail!("{} has no version_url configured, cannot list remote versions", sdk_name);
        }

        // 拉取时显示 spinner
        let pb = InstallProgress::new_resolve(&sdk_name, "");
        let source = VersionSource {
            primary_url,
            secondary_url,
        };

        // Python 备源是 GitHub API → 需要 Accept header
        let headers = if let Sdk::Built(BuiltinSdk::Python) = sdk {
            Some(HashMap::from([(
                "Accept".to_string(),
                "application/vnd.github+json".to_string(),
            )]))
        } else {
            None
        };

        // 复用 version 模块的"缓存优先 + fetch"管线
        let body = fetch_version_data(
            &client,
            &source,
            &cache_key,
            &sdk_name,
            headers,
            self.config.network.cache_ttl_secs as u64,
        )
        .await?;
        pb.finish_with_message(format!("✅ Fetched remote versions for {}", sdk_name));

        let entries = strategy.parse_version_data(&body)?;

        // 用本地安装状态补充信息
        let local_versions = self.list_local_sdk_versions(sdk)?;
        let current_version = sdk_conf.current_version.as_deref();

        let items: Vec<RemoteVersionItem> = entries
            .iter()
            .map(|entry| {
                let is_local = local_versions.iter().any(|v| v.sdk_version == entry.full_version);
                let is_active = current_version == Some(&entry.full_version);
                let status = if is_active {
                    InstallStatus::Active
                } else if is_local {
                    InstallStatus::Installed
                } else {
                    InstallStatus::NotInstalled
                };
                RemoteVersionItem {
                    full_version: entry.full_version.clone(),
                    feature_version: entry.feature_version.clone(),
                    install_status: status,
                    source_url: source_display_url.to_string(),
                }
            })
            .collect();

        // 应用数量限制
        let max = limit as usize;
        let total_count = items.len();
        let result_items = if items.len() > max {
            warning!("{} has {} remote versions, showing first {}", sdk_name, items.len(), max);
            items[..max].to_vec()
        } else {
            items
        };

        Ok(RemoteVersionResult {
            items: result_items,
            total_count,
        })
    }

    /// 同步桥接:拉取远程版本列表数据(供 TUI 选择器使用)
    pub fn fetch_remote_version_list(&self, sdk: &Sdk, limit: u32) -> Result<RemoteVersionResult> {
        let rt = try_bug!(tokio::runtime::Runtime::new().context("Failed to create tokio runtime"));
        rt.block_on(self.fetch_remote_versions_async(sdk, limit))
    }
}
