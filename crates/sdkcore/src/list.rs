use crate::install::downloader::build_reqwest_client;
use crate::install::progress::InstallProgress;
use crate::manager::SdkManager;
use crate::size_cache::SizeCache;
use crate::version::{VersionSource, fetch_version_data, get_version_discovery};
use anyhow::{Context, Result, bail};
use crossterm::{
    cursor, execute,
    style::Stylize,
    terminal::{Clear, ClearType},
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};
use std::iter::once;
use std::path::PathBuf;
use std::str::FromStr;
use unicode_width::UnicodeWidthStr;
use util::consts::{DIVIDER, STATUS_ACTIVE};
use util::path::{format_bytes, get_installed_sdks_dir, get_sdkm_home};
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

// ─── 本地列表 ────────────────────────────────────────────────────

/// 远程版本列表结果(含截断前的总数)
pub struct RemoteVersionResult {
    pub items: Vec<RemoteVersionItem>,
    /// 应用数量限制前的总数(用于 TUI 头部展示)
    pub total_count: usize,
}

/// 打印本地 SDK 列表（居中标题 + 分隔线 + 表头 + 数据行），返回已打印的终端行数
///
/// 标题：居中、带 ℹ️ 图标、小写、无冒号、蓝色粗体；标题下一条分隔线隔开表格。
/// 抽出供 `show_local_sdk_list` 冷路径重绘：先打骨架（size="…"）→ 算完 →
/// 光标回退 N 行 + 清屏向下 → 再打最终表。返回行数用于光标回退量。
fn print_local_table(items: &[(String, String, String)]) -> u16 {
    // 居中标题（无顶部 divider，标题下加 divider 与表格分隔）
    let title = "ℹ️  installed sdks";
    // ℹ️ 带 VS16 在多数终端按 emoji 渲染宽 2，unicode-width 仅算 1 → 补 1，否则居中偏右
    let title_w = UnicodeWidthStr::width(title) + 1;
    let left_pad = DIVIDER.chars().count().saturating_sub(title_w) / 2;
    println!("{}{}", " ".repeat(left_pad), title.blue().bold());
    divider!();
    // 列头用 total 而非 size：概览每行是一个 SDK，size 列 = 该 SDK 全部版本总占用（非 current 版本大小），
    // 用 total 避免与 current 版本号紧邻造成的"current 版本的大小"误读
    let headers = ["sdk", "current", "total"];
    let active_w = UnicodeWidthStr::width(STATUS_ACTIVE);
    // 数据行前缀 = `{status} {idx:>2}. `，表头行用同等宽度空格缩进对齐数据列
    let prefix_w = active_w + 1 + 2 + 2;
    // 列宽取表头与数据中的最大显示宽度
    let sdk_w = items
        .iter()
        .map(|(s, _, _)| s.as_str().width())
        .chain(once(headers[0].width()))
        .max()
        .unwrap_or(0);
    let ver_w = items
        .iter()
        .map(|(_, v, _)| v.as_str().width())
        .chain(once(headers[1].width()))
        .max()
        .unwrap_or(0);
    let size_w = items
        .iter()
        .map(|(_, _, z)| z.as_str().width())
        .chain(once(headers[2].width()))
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
    // 行数 = 1(标题) + 1(分隔线) + 1(表头) + items.len()
    (3 + items.len()) as u16
}

impl SdkManager {
    /// 打印所有已安装 SDK 及其当前版本(非交互式摘要)
    ///
    /// 居中标题 + 分隔线 + 表格（无顶部分割线，底部一条收尾）：
    /// ```text
    ///            ℹ️  installed sdks
    /// ──────────────────────────────────────────────────
    ///        sdk     current  total
    /// ✅  1. java    8.0.492  518.3 MB
    /// ──────────────────────────────────────────────────
    /// ```
    pub fn show_local_sdk_list(&self) -> Result<()> {
        let sdk_dir = get_installed_sdks_dir()?;
        // 收集行：(sdk_name, current, dir_path)，size 延后由缓存解析（不再构造时同步算）
        let mut rows: Vec<(String, String, PathBuf)> = Vec::new();
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
            rows.push((sdk_name, current, path));
        }

        if rows.is_empty() {
            info!("no installed sdks. run `sdkm install <sdk> <version>` to get started.");
            return Ok(());
        }

        // 缓存优先：先查命中（不计算），全命中直接打印；有未命中走冷路径渐进
        let mut cache = SizeCache::load();
        cache.prune();
        let cached: Vec<Option<u64>> = rows.iter().map(|(_, _, p)| cache.cached(p)).collect();
        let any_miss = cached.iter().any(Option::is_none);

        let final_items: Vec<(String, String, String)> = if !any_miss {
            // 热路径：全命中，直接组最终表（无延迟）
            rows.iter()
                .zip(cached)
                .map(|((s, c, _), sz)| (s.clone(), c.clone(), format_bytes(sz.unwrap_or(0))))
                .collect()
        } else {
            // 冷路径：先打骨架（size 列 "…" 提示，先展示 sdk/version），算完重绘成最终表
            let tty = io::stdout().is_terminal();
            if tty {
                let skeleton: Vec<(String, String, String)> =
                    rows.iter().map(|(s, c, _)| (s.clone(), c.clone(), "…".to_string())).collect();
                let printed = print_local_table(&skeleton);
                let _ = io::stdout().flush();
                // 计算（resolve 命中即返，未命中 jwalk 并行 + 回写）
                let sizes: Vec<u64> = rows.iter().map(|(_, _, p)| cache.resolve(p)).collect();
                // 重绘：光标回表头行，清屏向下，打最终表
                let mut stdout = io::stdout();
                let _ = execute!(stdout, cursor::MoveToPreviousLine(printed), Clear(ClearType::FromCursorDown),);
                let _ = stdout.flush();
                rows.iter()
                    .zip(sizes)
                    .map(|((s, c, _), b)| (s.clone(), c.clone(), format_bytes(b)))
                    .collect()
            } else {
                // 管道/非 TTY：算完再打（无渐进，避免 ANSI 污染管道）
                let sizes: Vec<u64> = rows.iter().map(|(_, _, p)| cache.resolve(p)).collect();
                rows.iter()
                    .zip(sizes)
                    .map(|((s, c, _), b)| (s.clone(), c.clone(), format_bytes(b)))
                    .collect()
            }
        };

        cache.save();
        print_local_table(&final_items);
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
