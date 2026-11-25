//! 后台任务层：GUI 分层约定——每操作独立构造 Manager+Sink，在进程级共享 runtime 上执行
//! （core 只出 async API，runtime 归前端持有），只跨线程传事件/结果。
//!
//! 本地重段（全盘体积扫描/卸载删除）经 spawn_blocking 放到阻塞线程池，不占用 runtime worker。
//! 操作类任务（切换/卸载/安装）完成后回传 `TaskOutcome::Done`，UI 端据此刷新列表。

use crate::platform::open_path;
use crate::sink::{GuiEvent, GuiSink};
use sdkm_core::manager::SdkManager;
use sdkm_core::self_update::SelfUpdateOutcome;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::spawn_blocking;

/// 任务标识（UI 端按它分发结果到对应页面状态）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskId {
    /// 概览页数据刷新
    OverviewRefresh,
    /// SDK 管理页：注册 SDK 列表（含本地版本列表）
    SdkListRefresh,
    /// SDK 管理页：单 SDK 远程版本列表
    SdkRemoteList { name: String },
    /// SDK 管理页：切换版本
    SdkSwitch { name: String, version: String },
    /// SDK 管理页：卸载版本
    SdkUninstall { name: String, version: String },
    /// SDK 管理页：安装版本（先解析）
    SdkInstallResolve { name: String, version: String },
    /// SDK 管理页：执行安装（版本已确认，内部再精确解析后安装）
    SdkInstallExec { name: String, version: String },
    /// SDK 管理页：导入本地版本目录（移动进 store 托管）
    SdkImportLocal {
        name: String,
        version: String,
        src_dir: String,
    },
    /// 选择要导入的本地版本目录（系统目录选择弹窗）
    PickImportDir { title: String },
    /// 注册自定义 SDK（表单字段）
    ConfigAddSdk {
        name: String,
        version_url: Option<String>,
        download_url: Option<String>,
        bin_dir: Option<String>,
    },
    /// 移除自定义 SDK
    ConfigRemoveSdk { name: String },
    /// 保存网络设置（config.toml network 子表）
    ConfigSaveNetwork {
        proxy: Option<String>,
        ssl_verify: bool,
        timeout: u32,
        ttl: u32,
        token: Option<String>,
        mirror: Option<String>,
    },
    /// 读取项目 .sdkm.toml（dir 为项目目录）
    ProjectLoad { dir: String },
    /// 保存项目 .sdkm.toml（pins；空 = 清除全部固定）
    ProjectSave { dir: String, pins: Vec<(String, String)> },
    /// 进入项目管理页时自动加载（当前工作目录向上找 .sdkm.toml）
    ProjectAutoLoad,
    /// 读取网络设置（设置页表单回填）
    ConfigLoad,
    /// 人工重新加载网络设置（设置页「重新加载」，完成后给提示）
    ConfigReload,
    /// 检查 sdkm 更新（只查最新版本，不下载）
    UpdateCheck,
    /// 查询回退目标版本（只读，用于确认弹窗）
    UpdatePreviousVersion,
    /// 执行 sdkm 更新（下载替换，进度经 Core 事件）
    UpdateRun,
    /// 回滚 sdkm 版本（确认后）
    UpdateRollback,
    /// 首次初始化 / 环境修复（不覆盖已有 config）
    InitRun,
    /// 重新初始化（重置 config，确认后）
    Reinit,
    /// 卸载 sdkm（确认后）
    UninstallSelf,
    /// 系统目录选择弹窗（项目目录；title 为弹窗标题，随语言切换）
    PickDirectory { title: String },
    /// 系统目录选择弹窗（符号链接目录）
    PickSymlinkDir { title: String },
    /// 保存符号链接目录（path 空 = 跟随 Home）
    ConfigSaveSymlink { path: String },
    /// 用系统默认程序打开 config.toml
    OpenConfigPath,
    /// 用系统默认程序打开任意文件（项目 .sdkm.toml 等）
    OpenFile { path: String },
    /// 读取各 SDK 的版本源/下载源（SDK 源设置回填）
    SdkSourcesLoad,
    /// 人工重新加载 SDK 源（设置页「重新加载」，完成后给提示）
    SdkSourcesReload,
    /// 批量保存各 SDK 源配置（None/空串 = 清空）
    SdkSourcesSaveFull { entries: Vec<SdkSourceEntry> },
    /// 环境健康检查（PATH / CLI / 配置 / 存储 / 链接）
    DiagEnv,
    /// 清理缓存与临时目录（诊断页「清除缓存数据」）
    CleanData,
    /// Shell hook 注册状态检测
    ShellStatus,
    /// 注入 Shell hook
    HookInject,
}

/// 后台任务结果（跨线程传给 UI）
#[derive(Debug, Clone)]
pub enum TaskOutcome {
    /// 概览页数据包（行 + 尺寸 + 初始化状态）
    Overview(Result<OverviewData, String>),
    /// 注册 SDK 列表
    SdkList(Result<Vec<SdkListItem>, String>),
    /// 远程版本列表
    RemoteList { result: Result<Vec<RemoteItem>, String> },
    /// 安装解析结果（fuzzy=true 需 UI 确认后再执行安装）
    ResolvedForInstall {
        name: String,
        full_version: String,
        fuzzy: bool,
    },
    /// 操作类任务完成且成功（失败走 Failed；错误文案由 Core Error 事件展示）
    Done,
    /// 操作类任务失败（错误已由 Core Error 事件提示；UI 只做解除忙碌 + 刷新）
    Failed,
    /// 导入目录选择结果
    ImportDirPicked(Option<String>),
    /// 项目 pins（path = .sdkm.toml 完整路径）
    ProjectPins {
        dir: String,
        path: String,
        pins: Vec<(String, String)>,
    },
    /// 自动加载未找到项目（保持当前状态）
    ProjectAutoMiss,
    /// 网络设置当前值（表单回填）
    ConfigValues {
        proxy: String,
        ssl_verify: bool,
        timeout: u32,
        ttl: u32,
        token: String,
        mirror: String,
        symlink_dir: String,
    },
    /// 系统目录选择结果
    DirectoryPicked(Option<String>),
    /// 符号链接目录选择结果
    SymlinkDirPicked(Option<String>),
    /// 符号链接目录已保存（新值回显；空串 = 跟随 Home）
    SymlinkSaved(String),
    /// SDK 源配置
    SdkSources(Vec<SdkSourceEntry>),
    /// 最新远程版本号（检查更新结果；失败为 Err 文案）
    LatestVersion(Result<String, String>),
    /// 回退目标版本
    PreviousVersion(Result<String, String>),
    /// 自更新成功结果（是否发生替换由 core 返回）
    SelfUpdated(SelfUpdateOutcome),
    /// 自更新失败（错误已经由 Error 事件展示，这里只用于解除忙碌态）
    UpdateFailed,
    /// 初始化/修复结果（错误在任务内捕获，供引导页与诊断页展示）
    InitResult(Result<(), String>),
    /// 环境健康检查结果
    EnvStatus(Result<sdkm_core::doctor::EnvironmentStatus, String>),
    /// 缓存/临时目录清理结果（Ok = 释放字节数）
    CleanDone(Result<u64, String>),
    /// Shell hook 状态
    ShellStatus(Result<ShellStatus, String>),
    /// Shell hook 注入结果
    HookInjected(Result<(), String>),
}

/// 设置页 SDK 源条目（4 个地址 + bin/extras，自定义 SDK 编辑用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkSourceEntry {
    pub name: String,
    pub version_url: Option<String>,
    pub version_fallback_url: Option<String>,
    pub download_url: Option<String>,
    pub download_fallback_url: Option<String>,
    /// 二进制子目录（None = 根目录）
    pub bin_dir: Option<String>,
    /// 环境变量表（如 JAVA_HOME={sdk_dir}）
    pub extra_vars: Vec<(String, String)>,
    /// 额外 PATH 条目
    pub extra_paths: Vec<String>,
}

/// Shell hook 状态（GUI 展示结构）
#[derive(Debug, Clone)]
pub struct ShellStatus {
    pub shell: String,
    pub profiles: Vec<(String, bool)>,
    pub all_registered: bool,
}

/// 概览页数据包
#[derive(Debug, Clone)]
pub struct OverviewData {
    pub rows: Vec<SdkListItem>,
    /// (sdk_name, bytes)，仅已安装项，按大小降序
    pub sizes: Vec<(String, u64)>,
    pub total_size: u64,
    /// config.toml 存在且 store 目录存在 = 已初始化
    pub init_ok: bool,
}

/// SDK 行（概览页 / SDK 管理页共用：全部注册 SDK，含未安装项）
#[derive(Debug, Clone)]
pub struct SdkListItem {
    pub name: String,
    pub installed: bool,
    pub current: Option<String>,
    pub has_version_url: bool,
    /// 是否具备远程安装能力（core 统一口径：下载模板或版本发现源至少有一个）
    pub can_install: bool,
    /// 种子表内置（含引擎型内置）；不在种子表的才是用户自定义
    pub is_builtin: bool,
    /// 本地已安装版本列表（未安装为空）
    pub versions: Vec<String>,
}

/// 远程版本条目（GUI 展示结构）
#[derive(Debug, Clone)]
pub struct RemoteItem {
    pub full_version: String,
    pub installed: bool,
    pub active: bool,
}

/// GUI 进程级共享 runtime：所有后台任务的执行环境。
/// 常驻 runtime 是连接复用（core 内 client 缓存）与即时取消的前提；
/// worker=2 足够——任务多为网络等待，本地重段走 spawn_blocking。
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}

/// 派发后台任务：共享 runtime + 独立 Manager/GuiSink（GUI 分层硬约束）
///
/// 结果统一经 channel 以 `GuiEvent::Task` 回传；core 过程事件/进度由 GuiSink 同道转发。
pub fn spawn_task(tx: UnboundedSender<GuiEvent>, id: TaskId) {
    let _ = spawn_task_inner(tx, id, None);
}

/// 可取消任务（安装等长任务；GUI 关闭进度弹窗时置位）
/// 返回 JoinHandle：UI 关闭进度弹窗时 abort，让网络段在下一个 await 点立即终止
pub fn spawn_task_with_cancel(
    tx: UnboundedSender<GuiEvent>,
    id: TaskId,
    cancel: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    spawn_task_inner(tx, id, Some(cancel))
}

fn spawn_task_inner(
    tx: UnboundedSender<GuiEvent>,
    id: TaskId,
    cancel: Option<Arc<AtomicBool>>,
) -> tokio::task::JoinHandle<()> {
    let sink: Arc<dyn sdkm_core::report::EventSink> = Arc::new(GuiSink::with_cancel(tx.clone(), cancel.clone()));
    runtime().spawn(async move {
        let outcome = build_outcome(&sink, &tx, &id, cancel.as_deref()).await;
        if let Some(outcome) = outcome {
            let _ = tx.send(GuiEvent::Task { id, outcome });
        }
    })
}

/// 按 TaskId 执行任务并产出结果（None = 无 outcome，过程/成败走 Core 事件）
async fn build_outcome(
    sink: &Arc<dyn sdkm_core::report::EventSink>,
    tx: &UnboundedSender<GuiEvent>,
    id: &TaskId,
    cancel: Option<&AtomicBool>,
) -> Option<TaskOutcome> {
    let result = build_task_result(sink, id).await;
    match result {
        Ok(outcome) => Some(outcome),
        Err(e) => {
            // 失败：错误经 Core Error 事件展示；操作类回 Failed 让 UI 解除忙碌态并刷新（不弹成功提示）
            // 用户主动取消不提示错误（避免取消后弹红色提示）
            if !cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
                let _ = tx.send(GuiEvent::Core(sdkm_core::report::CoreEvent::Error(format!("{e:#}"))));
            }
            match id {
                TaskId::SdkSwitch { .. }
                | TaskId::SdkUninstall { .. }
                | TaskId::SdkInstallResolve { .. }
                | TaskId::SdkInstallExec { .. }
                | TaskId::SdkImportLocal { .. }
                | TaskId::ConfigAddSdk { .. }
                | TaskId::ConfigRemoveSdk { .. }
                | TaskId::ConfigSaveNetwork { .. }
                | TaskId::ConfigSaveSymlink { .. }
                | TaskId::ProjectSave { .. }
                | TaskId::UninstallSelf
                | TaskId::ConfigLoad
                | TaskId::ConfigReload
                | TaskId::OpenConfigPath
                | TaskId::OpenFile { .. }
                | TaskId::SdkSourcesSaveFull { .. } => Some(TaskOutcome::Failed),
                TaskId::ProjectLoad { dir } => Some(TaskOutcome::ProjectPins {
                    dir: dir.clone(),
                    path: String::new(),
                    pins: Vec::new(),
                }),
                TaskId::ProjectAutoLoad => Some(TaskOutcome::ProjectAutoMiss),
                TaskId::DiagEnv => Some(TaskOutcome::EnvStatus(Err(format!("{e:#}")))),
                TaskId::CleanData => Some(TaskOutcome::CleanDone(Err(format!("{e:#}")))),
                TaskId::ShellStatus => Some(TaskOutcome::ShellStatus(Err(format!("{e:#}")))),
                TaskId::HookInject => Some(TaskOutcome::HookInjected(Err(format!("{e:#}")))),
                TaskId::UpdateCheck => Some(TaskOutcome::LatestVersion(Err(format!("{e:#}")))),
                TaskId::UpdatePreviousVersion => Some(TaskOutcome::PreviousVersion(Err(format!("{e:#}")))),
                TaskId::UpdateRun | TaskId::UpdateRollback => Some(TaskOutcome::UpdateFailed),
                TaskId::InitRun | TaskId::Reinit => Some(TaskOutcome::InitResult(Err(format!("{e:#}")))),
                TaskId::PickDirectory { .. } => Some(TaskOutcome::DirectoryPicked(None)),
                TaskId::PickSymlinkDir { .. } => Some(TaskOutcome::SymlinkDirPicked(None)),
                TaskId::PickImportDir { .. } => Some(TaskOutcome::ImportDirPicked(None)),
                TaskId::SdkSourcesLoad | TaskId::SdkSourcesReload => Some(TaskOutcome::Failed),
                TaskId::OverviewRefresh => Some(TaskOutcome::Overview(Err(format!("{e:#}")))),
                TaskId::SdkListRefresh => Some(TaskOutcome::SdkList(Err(format!("{e:#}")))),
                TaskId::SdkRemoteList { .. } => Some(TaskOutcome::RemoteList {
                    result: Err(format!("{e:#}")),
                }),
            }
        }
    }
}

/// 任务执行体：构造 Manager 并调用 core（网络段 async 直调；本地重段走阻塞线程池）
async fn build_task_result(sink: &Arc<dyn sdkm_core::report::EventSink>, id: &TaskId) -> anyhow::Result<TaskOutcome> {
    // 初始化/修复不能先构造 SdkManager：首次初始化时 config.toml 还不存在，
    // SdkManager::new 会直接报“Failed to read sdkm config”。这里先行分流。
    if matches!(id, TaskId::InitRun | TaskId::Reinit) {
        let force = matches!(id, TaskId::Reinit);
        let result = SdkManager::init_sdkm(sink.clone(), force)
            .map(|_| ())
            .map_err(|e| format!("{e:#}"));
        return Ok(TaskOutcome::InitResult(result));
    }

    let mut manager = SdkManager::new(sink.clone())?;
    match id.clone() {
        // ── 概览数据包 ──
        TaskId::OverviewRefresh => {
            // 全盘体积扫描（jwalk）是本地重段，放阻塞线程池避免占用 runtime worker
            let data = spawn_blocking(move || overview_data(&manager))
                .await
                .map_err(|e| format!("{e}"))
                .and_then(|r| r.map_err(|e| format!("{e:#}")));
            Ok(TaskOutcome::Overview(data))
        }
        // ── SDK 列表（与概览页共用数据源）──
        TaskId::SdkListRefresh => {
            // store 目录逐版本枚举走阻塞线程池，避免占用 runtime worker
            let list = spawn_blocking(move || collect_sdk_rows(&manager))
                .await
                .map_err(|e| format!("{e}"))
                .and_then(|r| r.map_err(|e| format!("{e:#}")));
            Ok(TaskOutcome::SdkList(list))
        }
        // ── 远程版本列表 ──
        TaskId::SdkRemoteList { name } => {
            let sdk = parse_sdk(&name)?;
            let res = manager
                .fetch_remote_versions_async(&sdk, u32::MAX)
                .await
                .map(|r| {
                    r.items
                        .into_iter()
                        .map(|it| RemoteItem {
                            installed: matches!(
                                it.install_status,
                                sdkm_core::list::InstallStatus::Installed | sdkm_core::list::InstallStatus::Active
                            ),
                            active: matches!(it.install_status, sdkm_core::list::InstallStatus::Active),
                            full_version: it.full_version,
                        })
                        .collect()
                })
                .map_err(|e| format!("{e:#}"));
            let _ = name;
            Ok(TaskOutcome::RemoteList { result: res })
        }
        // ── 操作类：core 执行，成败经事件；完成回 Done ──
        TaskId::SdkSwitch { name, version } => {
            let sdk = parse_sdk(&name)?;
            manager.switch_sdk_to_version(&sdk, &version)?;
            Ok(TaskOutcome::Done)
        }
        TaskId::SdkUninstall { name, version } => {
            let sdk = parse_sdk(&name)?;
            // 删除版本目录（秒级磁盘操作）走阻塞线程池
            spawn_blocking(move || manager.uninstall_sdk(&sdk, &version))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))??;
            Ok(TaskOutcome::Done)
        }
        TaskId::SdkImportLocal { name, version, src_dir } => {
            let sdk = parse_sdk(&name)?;
            // 目录移动（跨盘时是 copy+delete）是本地重段，走阻塞线程池
            spawn_blocking(move || manager.import_local_version(&sdk, Path::new(&src_dir), &version))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))??;
            Ok(TaskOutcome::Done)
        }
        TaskId::ConfigAddSdk {
            name,
            version_url,
            download_url,
            bin_dir,
        } => {
            let sdk_conf = sdkm_core::config::SdkConfig {
                name,
                version_url,
                version_fallback_url: None,
                download_url,
                download_fallback_url: None,
                current_version: None,
                bin_dir,
                extra_vars: Default::default(),
                extra_paths: Vec::new(),
                os_style: None,
                arch_style: None,
                add_to_path: None,
            };
            manager.config.add_sdk(sdk_conf, sink.as_ref())?;
            Ok(TaskOutcome::Done)
        }
        TaskId::ConfigRemoveSdk { name } => {
            manager.config.remove_sdk(&name, sink.as_ref())?;
            Ok(TaskOutcome::Done)
        }
        TaskId::ConfigSaveNetwork {
            proxy,
            ssl_verify,
            timeout,
            ttl,
            token,
            mirror,
        } => {
            use sdkm_core::config::{ConfigKey, ValidatedValue};
            // 空值 = 用户清空该项：走 delete 语义（proxy 存空串会让 reqwest 代理构建失败）
            let cfg = &mut manager.config;
            let sink_ref = sink.as_ref();
            match proxy {
                Some(p) => cfg.set_value(&ConfigKey::NetworkProxy, ValidatedValue::FreeString(p), sink_ref)?,
                None => cfg.delete_value(&ConfigKey::NetworkProxy, sink_ref)?,
            }
            cfg.set_value(&ConfigKey::NetworkSslVerify, ValidatedValue::Bool(ssl_verify), sink_ref)?;
            cfg.set_value(&ConfigKey::NetworkConnectTimeout, ValidatedValue::U32(timeout), sink_ref)?;
            cfg.set_value(&ConfigKey::NetworkCacheTtlSecs, ValidatedValue::U32(ttl), sink_ref)?;
            match token {
                Some(t) => cfg.set_value(&ConfigKey::NetworkGithubToken, ValidatedValue::Token(t), sink_ref)?,
                None => cfg.delete_value(&ConfigKey::NetworkGithubToken, sink_ref)?,
            }
            match mirror {
                Some(m) => {
                    cfg.set_value(&ConfigKey::NetworkGithubDownloadMirror, ValidatedValue::FreeString(m), sink_ref)?
                }
                None => cfg.delete_value(&ConfigKey::NetworkGithubDownloadMirror, sink_ref)?,
            }
            Ok(TaskOutcome::Done)
        }
        TaskId::ProjectAutoLoad => {
            // 优先 hook 缓存里最近使用过的项目（key = 项目目录）
            if let Some(p) = sdkm_core::hook_cache::cached_projects().first() {
                let cfg = sdkm_core::project_config::read_project_config(std::path::Path::new(&p.config_path));
                return Ok(TaskOutcome::ProjectPins {
                    dir: p.dir.clone(),
                    path: p.config_path.clone(),
                    pins: cfg.pins.into_iter().collect(),
                });
            }
            // 回退：当前工作目录向上找 .sdkm.toml
            match sdkm_core::project_config::find_project_config()? {
                Some((path, cfg)) => {
                    let dir = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    Ok(TaskOutcome::ProjectPins {
                        dir,
                        path: path.to_string_lossy().into_owned(),
                        pins: cfg.pins.into_iter().collect(),
                    })
                }
                None => Ok(TaskOutcome::ProjectAutoMiss),
            }
        }
        TaskId::ProjectLoad { dir } => {
            let dir_path = std::path::Path::new(&dir);
            let path = dir_path.join(".sdkm.toml");
            let cfg = sdkm_core::project_config::read_project_config(&path);
            let pins = cfg.pins.into_iter().collect::<Vec<_>>();
            Ok(TaskOutcome::ProjectPins {
                dir,
                path: path.to_string_lossy().into_owned(),
                pins,
            })
        }
        TaskId::ProjectSave { dir, pins } => {
            let map: std::collections::HashMap<String, String> = pins.into_iter().collect();
            let cfg = sdkm_core::project_config::pins_from_map(map);
            sdkm_core::project_config::write_project_config(std::path::Path::new(&dir), &cfg)?;
            Ok(TaskOutcome::Done)
        }
        TaskId::ConfigLoad | TaskId::ConfigReload => {
            use sdkm_core::config::ConfigKey;
            let get = |key| manager.config.get_value(key).unwrap_or_default();
            Ok(TaskOutcome::ConfigValues {
                proxy: get(&ConfigKey::NetworkProxy),
                ssl_verify: manager.config.network.ssl_verify,
                timeout: manager.config.network.connect_timeout,
                ttl: manager.config.network.cache_ttl_secs,
                token: manager.config.network.github_token.clone().unwrap_or_default(),
                mirror: get(&ConfigKey::NetworkGithubDownloadMirror),
                symlink_dir: manager.config.resolved_symlink_dir().unwrap_or_default(),
            })
        }
        TaskId::DiagEnv => Ok(TaskOutcome::EnvStatus(Ok(sdkm_core::doctor::collect_environment_status(
            &manager.config,
        )))),
        TaskId::CleanData => {
            // 体积统计 + 删除是本地重段，走阻塞线程池
            let report = spawn_blocking(move || manager.clean(true, true))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))??;
            Ok(TaskOutcome::CleanDone(Ok(report.total())))
        }
        TaskId::ShellStatus => {
            let status = sdkm_core::shell::check_shell_hook().map(|st| ShellStatus {
                shell: st.shell.display_name().to_string(),
                all_registered: st.all_registered(),
                profiles: st
                    .profiles
                    .into_iter()
                    .map(|p| (p.path.display().to_string(), p.registered))
                    .collect(),
            });
            Ok(TaskOutcome::ShellStatus(status.map_err(|e| format!("{e:#}"))))
        }
        TaskId::HookInject => {
            let r = sdkm_core::shell::inject_shell_hook_checked(sink.as_ref())
                .map(|_| ())
                .map_err(|e| format!("{e:#}"));
            Ok(TaskOutcome::HookInjected(r))
        }
        TaskId::UpdateCheck => {
            let latest = manager.fetch_latest_version_async().await.map_err(|e| format!("{e:#}"));
            Ok(TaskOutcome::LatestVersion(latest))
        }
        TaskId::UpdatePreviousVersion => {
            let previous = manager.fetch_previous_version_async().await.map_err(|e| format!("{e:#}"));
            Ok(TaskOutcome::PreviousVersion(previous))
        }
        TaskId::UpdateRun => {
            let outcome = manager.update_self_async(false, false).await?;
            Ok(TaskOutcome::SelfUpdated(outcome))
        }
        TaskId::UpdateRollback => {
            let outcome = manager.update_self_async(false, true).await?;
            Ok(TaskOutcome::SelfUpdated(outcome))
        }
        // 已在构造 Manager 前分流，这里仅保持 match 穷尽
        TaskId::InitRun | TaskId::Reinit => unreachable!("handled before SdkManager::new"),
        TaskId::UninstallSelf => {
            // 卸载自身要删目录/文件，走阻塞线程池
            spawn_blocking(move || manager.uninstall_self())
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))??;
            Ok(TaskOutcome::Done)
        }
        TaskId::OpenConfigPath => {
            if let Ok(cfg) = sdkm_util::path::get_sdkm_config_path()
                && cfg.exists()
            {
                open_path(&cfg.to_string_lossy());
            }
            Ok(TaskOutcome::Done)
        }
        TaskId::OpenFile { path } => {
            if std::path::Path::new(&path).exists() {
                open_path(&path);
            }
            Ok(TaskOutcome::Done)
        }
        TaskId::PickDirectory { title } => {
            // 系统弹窗是用户驱动的阻塞等待，走阻塞线程池，避免占住 runtime worker
            let picked = spawn_blocking(move || pick_directory(&title))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(TaskOutcome::DirectoryPicked(picked))
        }
        TaskId::PickSymlinkDir { title } => {
            let picked = spawn_blocking(move || pick_directory(&title))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(TaskOutcome::SymlinkDirPicked(picked))
        }
        TaskId::PickImportDir { title } => {
            let picked = spawn_blocking(move || pick_directory(&title))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(TaskOutcome::ImportDirPicked(picked))
        }
        TaskId::ConfigSaveSymlink { path } => {
            manager.config.set_value(
                &sdkm_core::config::ConfigKey::SymlinkDir,
                sdkm_core::config::ValidatedValue::FreeString(path.clone()),
                sink.as_ref(),
            )?;
            Ok(TaskOutcome::SymlinkSaved(path))
        }
        TaskId::SdkSourcesLoad | TaskId::SdkSourcesReload => {
            let items = manager
                .config
                .sdks
                .iter()
                .map(|e| SdkSourceEntry {
                    name: e.name.clone(),
                    version_url: e.version_url.clone(),
                    version_fallback_url: e.version_fallback_url.clone(),
                    download_url: e.download_url.clone(),
                    download_fallback_url: e.download_fallback_url.clone(),
                    bin_dir: e.bin_dir.clone(),
                    // HashMap 无序：按变量名排序，保证列表与 config.toml 稳定
                    extra_vars: {
                        let mut vars: Vec<_> = e.extra_vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        vars.sort();
                        vars
                    },
                    extra_paths: e.extra_paths.clone(),
                })
                .collect();
            Ok(TaskOutcome::SdkSources(items))
        }
        TaskId::SdkSourcesSaveFull { entries } => {
            let updates: Vec<sdkm_core::config::SdkSourceUpdate> = entries
                .into_iter()
                .map(|e| sdkm_core::config::SdkSourceUpdate {
                    name: e.name,
                    version_url: e.version_url,
                    version_fallback_url: e.version_fallback_url,
                    download_url: e.download_url,
                    download_fallback_url: e.download_fallback_url,
                    // 整表替换语义：GUI 全量提交，草稿即真相
                    bin_dir: Some(e.bin_dir.unwrap_or_default()),
                    extra_vars: Some(e.extra_vars.into_iter().collect()),
                    extra_paths: Some(e.extra_paths),
                })
                .collect();
            manager.config.set_sdk_sources(&updates, sink.as_ref())?;
            Ok(TaskOutcome::Done)
        }
        TaskId::SdkInstallResolve { name, version } => {
            let sdk = parse_sdk(&name)?;
            let resolved = manager.resolve_install_version_async(&sdk, &version).await?;
            Ok(TaskOutcome::ResolvedForInstall {
                name,
                full_version: resolved.full_version,
                fuzzy: resolved.fuzzy_matched,
            })
        }
        TaskId::SdkInstallExec { name, version } => {
            let sdk = parse_sdk(&name)?;
            // 确认后的版本重新精确解析再安装（CLI 语义：默认切换全局版本）
            let resolved = manager.resolve_install_version_async(&sdk, &version).await?;
            manager.install_sdk_async(&sdk, &resolved, true).await?;
            Ok(TaskOutcome::Done)
        }
    }
}

/// 调系统文件选择弹窗（取消返回 None）
fn pick_directory(title: &str) -> Option<String> {
    #[cfg(windows)]
    {
        // 优先资源管理器同款选择器；COM 不可用时回退旧版 PowerShell 弹窗
        match pick_directory_modern(title) {
            Ok(picked) => picked,
            Err(()) => pick_directory_legacy(title),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args(["-e", &format!(r#"POSIX path of (choose folder with prompt "{title}")"#)])
            .output()
            .ok()?;
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (!s.is_empty()).then_some(s)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // 优先 zenity（GNOME），其次 kdialog（KDE）；都缺席则返回 None
        let zenity = std::process::Command::new("zenity")
            .args(["--file-selection", "--directory", &format!("--title={title}")])
            .output();
        if let Ok(out) = zenity {
            if out.status.success() {
                let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
                return (!s.is_empty()).then_some(s);
            }
        }
        let kdialog = std::process::Command::new("kdialog")
            .args(["--getexistingdirectory", ".", title])
            .output()
            .ok()?;
        let s = String::from_utf8(kdialog.stdout).ok()?.trim().to_string();
        (!s.is_empty()).then_some(s)
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}

/// Windows 现代目录选择器（IFileOpenDialog + FOS_PICKFOLDERS，资源管理器同款）
///
/// Ok(Some) = 选中；Ok(None) = 用户取消；Err = 选择器运行失败（调用方回退旧弹窗）
#[cfg(windows)]
fn pick_directory_modern(title: &str) -> Result<Option<String>, ()> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

    // 选择框是模态 shell 窗口：调用线程必须以 STA 初始化 COM
    if unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_err() {
        return Err(());
    }
    let picked = unsafe { pick_directory_com(title) };
    unsafe { CoUninitialize() };
    picked
}

#[cfg(windows)]
unsafe fn pick_directory_com(title: &str) -> Result<Option<String>, ()> {
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
    use windows::Win32::UI::Shell::{
        FILEOPENDIALOGOPTIONS, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
    };
    use windows::core::{HRESULT, HSTRING};

    // 用户取消（ERROR_CANCELLED）与真实失败要分开：只有真失败才回退第二个弹窗
    let is_cancel = |e: &windows::core::Error| e.code() == HRESULT::from_win32(ERROR_CANCELLED.0);

    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }.map_err(|_| ())?;
    let options = unsafe { dialog.GetOptions() }.map_err(|_| ())?;
    // FORCEFILESYSTEM：只允许选文件系统目录，虚拟位置取不到路径会表现为"选了没反应"
    unsafe { dialog.SetOptions(FILEOPENDIALOGOPTIONS(options.0 | FOS_PICKFOLDERS.0 | FOS_FORCEFILESYSTEM.0)) }
        .map_err(|_| ())?;
    let _ = unsafe { dialog.SetTitle(&HSTRING::from(title)) };
    if let Err(e) = unsafe { dialog.Show(None) } {
        return if is_cancel(&e) { Ok(None) } else { Err(()) };
    }
    let item = match unsafe { dialog.GetResult() } {
        Ok(item) => item,
        Err(e) => return if is_cancel(&e) { Ok(None) } else { Err(()) },
    };
    let raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.map_err(|_| ())?;
    let path = unsafe { raw.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void)) };
    Ok(path.filter(|p| !p.is_empty()))
}

/// 旧版 PowerShell 目录选择弹窗（COM 不可用时的兜底）
#[cfg(windows)]
fn pick_directory_legacy(title: &str) -> Option<String> {
    // 强制 UTF-8 输出：PowerShell 5.1 重定向 stdout 默认用 OEM 代码页（中文系统 GBK），
    // 中文路径按 UTF-8 解码会失败 → 整个选择结果丢失（表现为"选了没反应"）
    let script = format!(
        r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
Add-Type -AssemblyName System.Windows.Forms | Out-Null;
$f = New-Object System.Windows.Forms.FolderBrowserDialog;
$f.Description = '{title}';
if ($f.ShowDialog() -eq 'OK') {{ $f.SelectedPath }} else {{ '' }}"#
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script]);
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW：避免弹出 PowerShell 控制台窗口
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output()
        .ok()
        // 兜底 lossy 解码：编码仍异常时给核心层一个可诊断的路径（后续会报目录不存在）
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 构造 sdk（内置/自定义按名称自动判定）
fn parse_sdk(name: &str) -> anyhow::Result<sdkm_util::sdk::Sdk> {
    use std::str::FromStr as _;
    sdkm_util::sdk::Sdk::from_str(name)
}

/// SDK 行收集（全部注册 SDK + 本地版本列表 + 内置判定）——概览页/管理页共用数据源
fn collect_sdk_rows(manager: &SdkManager) -> anyhow::Result<Vec<SdkListItem>> {
    let registered = manager.list_registered_sdks()?;
    let mut rows = Vec::new();
    for it in registered {
        let versions = if it.installed {
            manager
                .list_local_sdk_versions(&it.sdk)?
                .into_iter()
                .map(|v| v.sdk_version)
                .collect()
        } else {
            Vec::new()
        };
        // 内置判定走种子表（含 bun/pnpm 等引擎型内置），不在种子里的才是自定义
        let is_builtin = sdkm_util::builtin::find_seed(&it.name).is_some();
        rows.push(SdkListItem {
            name: it.name,
            installed: it.installed,
            current: it.current,
            has_version_url: it.has_version_url,
            can_install: it.can_install,
            is_builtin,
            versions,
        });
    }
    // 与 CLI ls 一致：已安装的排前面
    rows.sort_by(|a, b| b.installed.cmp(&a.installed).then_with(|| a.name.cmp(&b.name)));
    Ok(rows)
}

/// 概览数据包收集（SDK 行 + SizeCache 尺寸 + 初始化状态）
fn overview_data(manager: &SdkManager) -> anyhow::Result<OverviewData> {
    let rows = collect_sdk_rows(manager)?;
    let store_dir = sdkm_util::path::get_installed_sdks_dir()?;
    let mut cache = sdkm_core::size_cache::SizeCache::load();
    // 与 CLI ls 一致：先清卸载残留的孤儿缓存条目
    cache.prune();
    let mut sizes = Vec::new();
    let mut total = 0u64;
    for row in &rows {
        if row.installed {
            let dir = store_dir.join(&row.name);
            let sz = cache.resolve(&dir);
            sizes.push((row.name.clone(), sz));
            total += sz;
        }
    }
    sizes.sort_by(|a, b| b.1.cmp(&a.1));
    cache.save();
    let init_ok = sdkm_util::path::get_sdkm_config_path()?.exists() && store_dir.exists();
    Ok(OverviewData {
        rows,
        sizes,
        total_size: total,
        init_ok,
    })
}
