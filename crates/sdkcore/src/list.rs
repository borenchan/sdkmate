use crate::install::downloader::build_reqwest_client;
use crate::manager::SdkManager;
use crate::report::ProgressKind;
use crate::version::github_releases::{ensure_per_page, github_api_headers, is_github_releases_url};
use crate::version::{VersionSource, fetch_version_data, get_version_discovery};
use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::str::FromStr;
use util::path::get_installed_sdks_dir;
use util::sdk::{BuiltinSdk, Sdk};
use util::try_bug;

// ─── 数据结构 ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct SdkVersionItem {
    pub sdk: Sdk,
    pub sdk_version: String,
    pub sdk_dir: PathBuf,
    pub is_active: bool,
}

impl SdkVersionItem {
    pub fn new(sdk: Sdk, sdk_dir: PathBuf, is_active: bool) -> Self {
        let cow = sdk_dir.file_name().unwrap_or(OsStr::new("(empty dir)")).to_string_lossy();
        Self {
            sdk,
            sdk_version: cow.to_string(),
            sdk_dir,
            is_active,
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

/// 已注册 SDK 概览条目(第一层 SDK 选择 TUI / 非 TTY 概览共用)
#[derive(Debug)]
pub struct RegisteredSdkItem {
    pub sdk: Sdk,
    /// config 注册名
    pub name: String,
    /// store 中是否已安装(存在版本目录)
    pub installed: bool,
    /// 当前激活版本(config current_version,未激活为 None)
    pub current: Option<String>,
    /// 有版本发现源 → 支持远程列表(TUI 跳转 -r)
    pub has_version_url: bool,
}

/// 本地概览行（数据层）：store 中已安装 SDK 的名称/current/目录，size 留给前端经 SizeCache 解析
#[derive(Debug)]
pub struct LocalOverviewRow {
    pub sdk_name: String,
    /// 当前激活版本（config current_version，未激活为 "N/A"）
    pub current: String,
    /// SDK 在 store 下的目录（供 SizeCache 计算 total size）
    pub dir: PathBuf,
}

// ─── 本地列表 ────────────────────────────────────────────────────

/// 远程版本列表结果(含截断前的总数)
pub struct RemoteVersionResult {
    pub items: Vec<RemoteVersionItem>,
    /// 应用数量限制前的总数(用于 TUI 头部展示)
    pub total_count: usize,
}

impl SdkManager {
    /// 收集已安装 SDK 概览行（数据层）：store 目录遍历 + current 解析，不含 size
    ///
    /// 供非 TTY 概览表与未来 GUI 复用；表格渲染与 SizeCache 渐进策略在 CLI 层。
    pub fn local_overview_rows(&self) -> Result<Vec<LocalOverviewRow>> {
        let sdk_dir = get_installed_sdks_dir()?;
        let mut rows: Vec<LocalOverviewRow> = Vec::new();
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
            rows.push(LocalOverviewRow {
                sdk_name,
                current,
                dir: path,
            });
        }
        Ok(rows)
    }

    /// 获取本地 SDK 版本列表（纯数据：store 无该 SDK 返回空列表，零输出）
    ///
    /// "未装"提示归调用端——本函数被高频跨路径复用（resolve 预检查 / switch/uninstall
    /// 执行段 / 项目层 env 注入 / TUI / ls），各自对空列表的语义不同（bail / 静默降级 /
    /// 展示提示）；在数据函数里发提示会污染 `eval "$(sdkm env)"` 的 stdout 捕获（hook 报
    /// command not found），并造成与调用端 bail 的重复提示
    pub fn list_local_sdk_versions(&self, sdk: &Sdk) -> Result<Vec<SdkVersionItem>> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdks_root_dir = get_installed_sdks_dir()?;
        let sdk_dir = sdks_root_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy() == sdk.to_string());
        let Some(sdk_dir) = sdk_dir else {
            return Ok(vec![]);
        };
        Ok(sdk_dir
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
            .collect())
    }

    /// 收集所有已注册 SDK 概览(config.sdks 全量 + store 安装状态 + current + 可否远程)
    ///
    /// 供第一层 SDK 选择 TUI 与非 TTY 概览共用;不依赖 store 目录遍历判断注册(自定义 SDK 未安装也列出)
    pub fn list_registered_sdks(&self) -> Result<Vec<RegisteredSdkItem>> {
        let sdks_root_dir = get_installed_sdks_dir()?;
        let mut items = Vec::with_capacity(self.config.sdks.len());
        for sdk_conf in &self.config.sdks {
            let sdk = Sdk::from_str(&sdk_conf.name)?;
            let installed = sdks_root_dir.join(&sdk_conf.name).is_dir();
            let current = sdk_conf.current_version.clone();
            let has_version_url = sdk_conf.version_url.as_deref().is_some_and(|u| !u.is_empty());
            items.push(RegisteredSdkItem {
                sdk,
                name: sdk_conf.name.clone(),
                installed,
                current,
                has_version_url,
            });
        }
        Ok(items)
    }

    // ─── 远程列表 ────────────────────────────────────────────────

    /// 异步:拉取远程版本——带 spinner 的核心逻辑
    ///
    /// 复用 version 模块的 fetch_version_data + parse_version_data 管线。
    /// 缓存优先 + TTL,确保后续调用即时响应。
    async fn fetch_remote_versions_async(&self, sdk: &Sdk, limit: u32) -> Result<RemoteVersionResult> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let client = build_reqwest_client(&self.config.network)?;
        let sdk_name = sdk.to_string();
        let cache_key = sdk_name.to_lowercase();

        // Maven 无 version_url → 不支持远程版本列表
        if let Sdk::Built(BuiltinSdk::Maven) = sdk {
            bail!("Maven does not support remote version listing. Specify an exact version to install.");
        }

        // 版本源统一读 config（内置 SDK 种子已由 ensure_builtin_sdks 物化，用户可换源/镜像）
        let version_url = sdk_conf.version_url.clone().unwrap_or_default();
        let secondary_url = sdk_conf.version_fallback_url.clone();
        let source_display_url = if version_url.is_empty() {
            "N/A".to_string()
        } else {
            version_url.clone()
        };

        // 无版本发现源 → 无法列出远程版本
        if version_url.is_empty() && secondary_url.as_ref().is_none_or(|s: &String| s.is_empty()) {
            bail!("{} has no version_url configured, cannot list remote versions", sdk_name);
        }

        // 拉取时显示 spinner（GH 版本源自动补 per_page=100）；空版本输入 → 拉取列表文案
        let pb = self
            .sink
            .progress(ProgressKind::Resolve, format!("Fetching remote versions for {}...", sdk_name));
        let primary_url = ensure_per_page(&version_url);
        let source = VersionSource {
            primary_url,
            secondary_url,
        };

        // GitHub API 请求统一注入 Accept header（主源或备源任一为 GH API 即注入）
        let headers = if is_github_releases_url(&version_url)
            || sdk_conf
                .version_fallback_url
                .as_ref()
                .is_some_and(|u| is_github_releases_url(u))
        {
            Some(github_api_headers())
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
            &self.config.network,
            self.sink.as_ref(),
        )
        .await?;
        pb.finish_with_message(format!("✅ Fetched remote versions for {}", sdk_name));

        let strategy = get_version_discovery(sdk, &version_url);
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
            self.sink.warning(format!(
                "{} has {} remote versions, showing first {}",
                sdk_name,
                items.len(),
                max
            ));
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
