//! 根视图：侧边导航 + 标题栏 + 页面路由 + 后台事件泵
//!
//! 架构：单一 Entity（GuiApp）承载全部页面状态（切页不丢状态），页面渲染/事件
//! 处理拆分到 pages/ 模块；core 调用全部走后台线程（spawn_task），事件经单一
//! channel 泵回 UI 线程分发。

use crate::components::fmt_bytes;
use crate::data::{TaskId, TaskOutcome, spawn_task};
use crate::i18n::Lang;
use crate::pages::diagnostics::DiagnosticsState;
use crate::pages::overview::OverviewState;
use crate::pages::projects::ProjectsState;
use crate::pages::sdk::SdkPageState;
use crate::pages::settings::SettingsState;
use crate::pages::{diagnostics, onboarding, overview, projects, sdk, settings};
use crate::prefs::{GuiLang, GuiPrefs, GuiThemeMode, density, set_density};
use crate::sink::GuiEvent;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _, DropdownButton};
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sdkm_core::report::CoreEvent;
use sdkm_core::self_update::SelfUpdateOutcome;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

// 语言/主题/默认页面动作（下拉菜单触发，经窗口 action 冒泡到根视图处理）
actions!(
    sdkm_gui,
    [
        SetLangZh,
        SetLangEn,
        SetThemeSystem,
        SetThemeLight,
        SetThemeDark,
        SetDefaultOverview,
        SetDefaultSdk,
        SetDefaultProjects,
        SetDefaultSettings,
        SetDefaultDiagnostics
    ]
);

/// 轻提示类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Error,
    Warning,
    Success,
}

/// 轻提示（顶部居中，垂直堆叠，自动消失）
#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub msg: String,
}

/// 一级页面路由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Sdk,
    Projects,
    Settings,
    Diagnostics,
}

/// 系统信息（轻量本地查询，主线程一次性收集）
#[derive(Debug, Clone)]
pub struct SysInfo {
    pub os: String,
    pub arch: String,
    pub version: String,
    pub store_path: String,
    pub home_path: String,
}

impl SysInfo {
    fn collect() -> Self {
        Self {
            os: sdkm_util::diagnostics::os_version(),
            arch: sdkm_util::diagnostics::platform_info(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            store_path: sdkm_util::path::get_installed_sdks_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "—".into()),
            home_path: sdkm_util::path::get_sdkm_home()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "—".into()),
        }
    }
}

/// 根视图状态
pub struct GuiApp {
    pub prefs: GuiPrefs,
    pub lang: &'static Lang,
    pub active: Page,
    pub tx: UnboundedSender<GuiEvent>,
    pub sys_info: SysInfo,
    pub overview: OverviewState,
    pub sdk_page: SdkPageState,
    pub projects: ProjectsState,
    pub settings: SettingsState,
    pub diagnostics: DiagnosticsState,
    /// SDK 行数据（概览页/管理页共用一份，后台任务刷新）
    pub sdk_rows: Option<Vec<crate::data::SdkListItem>>,
    /// 活动进度（core 进度事件驱动；操作串行约定下单槽即可）
    pub progress: Option<ProgressState>,
    /// 操作中最近一条过程反馈（Detail/Warning/ProgressMessage 原文，进度弹窗单行展示）
    pub progress_note: Option<String>,
    /// 进度弹窗是否打开（过程事件按此分流：弹窗打开走单行反馈，否则走 toast）
    pub progress_dialog_open: bool,
    /// 最近拉取远程列表的 SDK（操作完成后自动重拉）
    pub last_remote: Option<String>,
    /// 进行中安装的取消标志（关闭进度弹窗时置位）
    pub install_cancel: Option<Arc<AtomicBool>>,
    /// 安装任务的 JoinHandle：关闭进度弹窗时 abort，网络段即时取消
    pub install_abort: Option<tokio::task::JoinHandle<()>>,
    /// 首次启动：config.toml 缺失，显示初始化引导
    pub needs_init: bool,
    /// 引导页初始化进行中
    pub initializing: bool,
    /// 引导页最近一次初始化错误（保持展示，便于用户重试）
    pub init_error: Option<String>,
    /// 顶部轻提示（垂直堆叠，自动消失）
    pub toasts: Vec<Toast>,
    next_toast_id: u64,
    /// 自更新已完成替换，等待重启生效（Some = 目标版本号）：
    /// 运行中进程仍是旧版，版本差异属预期，展示重启入口而非"版本不一致"
    pub restart_required: Option<String>,
    /// 启动时的 exe 路径（重启用）：自更新替换后 Linux 的 current_exe 会指向
    /// 已被改名/删除的 .discard 副本，必须用启动时记录的原始路径重新拉起
    pub exe_path: Option<PathBuf>,
    /// 订阅句柄（保持存活；搜索框 Change → 关键字等）
    pub _subscriptions: Vec<Subscription>,
}

/// 活动进度状态（下载 bytes / 解压计数等，SDK 页渲染）
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub kind: sdkm_core::report::ProgressKind,
    pub pos: u64,
    pub total: u64,
    /// 平滑速度（字节或条目/秒，EMA）；0 = 尚无采样（如刚开始下载）
    pub speed: f64,
    last_pos: u64,
    last_t: std::time::Instant,
}

impl ProgressState {
    fn new(kind: sdkm_core::report::ProgressKind) -> Self {
        Self {
            kind,
            pos: 0,
            total: 0,
            speed: 0.,
            last_pos: 0,
            last_t: std::time::Instant::now(),
        }
    }

    /// 位置采样：位置总是更新；速度按 ≥0.5s 窗口算瞬时值 EMA 平滑（α=0.3，首样本直采）
    fn sample(&mut self, pos: u64) {
        self.pos = pos;
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_t).as_secs_f64();
        if dt < 0.5 {
            return;
        }
        let inst = (pos.saturating_sub(self.last_pos)) as f64 / dt;
        self.speed = if self.speed <= 0. {
            inst
        } else {
            self.speed * 0.7 + inst * 0.3
        };
        self.last_pos = pos;
        self.last_t = now;
    }

    /// 预估剩余秒数（速度未知时返回 None）
    fn eta_secs(&self) -> Option<u64> {
        if self.speed <= 0. || self.total <= self.pos {
            return None;
        }
        Some(((self.total - self.pos) as f64 / self.speed).ceil() as u64)
    }
}

impl GuiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = GuiPrefs::load();
        set_density(prefs.density);
        let lang = Lang::pick(prefs.lang);
        let (tx, rx) = unbounded_channel();

        // 系统信息轻量收集（主线程一次本地 IO）
        let sys_info = SysInfo::collect();
        // 首次启动检测：config.toml 不存在 → 进入初始化引导，先不发业务请求
        let needs_init = sdkm_util::path::get_sdkm_config_path().map(|p| !p.is_file()).unwrap_or(true);

        let mut app = Self {
            prefs,
            lang,
            active: Page::Overview,
            tx: tx.clone(),
            sys_info,
            overview: OverviewState::new(window, cx, lang),
            sdk_page: SdkPageState::new(window, cx, lang),
            projects: ProjectsState::new(window, cx, lang),
            settings: SettingsState::new(window, cx, lang),
            diagnostics: DiagnosticsState::new(),
            sdk_rows: None,
            progress: None,
            progress_note: None,
            progress_dialog_open: false,
            last_remote: None,
            install_cancel: None,
            install_abort: None,
            needs_init,
            initializing: false,
            init_error: None,
            toasts: Vec::new(),
            next_toast_id: 1,
            restart_required: None,
            exe_path: env::current_exe().ok(),
            _subscriptions: Vec::new(),
        };

        // 概览搜索框：回车触发检索
        app._subscriptions.push(cx.subscribe_in(
            &app.overview.search,
            window,
            |app, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    app.overview.keyword = app.overview.search.read(cx).value().to_string();
                    app.overview.page = 0;
                    cx.notify();
                }
            },
        ));
        // SDK 管理搜索框：回车触发检索
        app._subscriptions.push(cx.subscribe_in(
            &app.sdk_page.search,
            window,
            |app, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    app.sdk_page.keyword = app.sdk_page.search.read(cx).value().to_string();
                    app.sdk_page.page = 0;
                    cx.notify();
                }
            },
        ));

        // 远程版本搜索：输入即过滤并回到第一页
        app._subscriptions.push(cx.subscribe_in(
            &app.sdk_page.remote_search,
            window,
            |app, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    app.sdk_page.remote_keyword = app.sdk_page.remote_search.read(cx).value().to_lowercase();
                    app.sdk_page.remote_page = 0;
                    cx.notify();
                }
            },
        ));

        // 项目管理页搜索：输入即过滤并回到第一页
        app._subscriptions.push(cx.subscribe_in(
            &app.projects.search,
            window,
            |app, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    app.projects.keyword = app.projects.search.read(cx).value().to_lowercase();
                    app.projects.page = 0;
                    cx.notify();
                }
            },
        ));

        // 设置页 SDK 源搜索：输入即过滤并回到第一页
        app._subscriptions.push(cx.subscribe_in(
            &app.settings.source_search,
            window,
            |app, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    app.settings.source_keyword = app.settings.source_search.read(cx).value().to_lowercase();
                    app.settings.source_page = 0;
                    cx.notify();
                }
            },
        ));

        // 窗口尺寸变化：强制整窗重绘 + 视图重渲染。绕过 gpui-pre 在特定
        // Windows 场景（DPI 切换/最大化还原）下布局停留在旧尺寸、窗口外溢区域
        // 未绘制的问题（症状：元素按旧尺寸排布，右下角透明）
        app._subscriptions.push(cx.observe_window_bounds(window, |_app, window, cx| {
            window.refresh();
            cx.notify();
        }));

        // 默认页面（偏好持久化）
        app.active = match app.prefs.default_page.as_str() {
            "sdk" => Page::Sdk,
            "projects" => Page::Projects,
            "settings" => Page::Settings,
            "diagnostics" => Page::Diagnostics,
            _ => Page::Overview,
        };

        // 首屏数据加载（概览 + SDK 列表）；未初始化时由引导页接管，避免无意义的报错请求
        if !app.needs_init {
            app.refresh_overview();
            spawn_task(tx.clone(), TaskId::SdkListRefresh);

            // 默认页直达设置/诊断时同样要触发首屏数据（navigate 不会被调用）
            if app.active == Page::Settings {
                spawn_task(tx.clone(), TaskId::ConfigLoad);
                spawn_task(tx.clone(), TaskId::SdkSourcesLoad);
            }
            if app.active == Page::Diagnostics {
                app.diagnostics.checking = true;
                spawn_task(tx.clone(), TaskId::DiagEnv);
                spawn_task(tx.clone(), TaskId::ShellStatus);
            }
            if app.active == Page::Projects {
                spawn_task(tx.clone(), TaskId::ProjectAutoLoad);
            }
        }

        // 事件泵：单一 channel 泵回 UI 线程分发；窗口关闭后自然退出
        cx.spawn(async move |this, cx| {
            pump_events(this, rx, cx).await;
        })
        .detach();

        app
    }

    /// SDK 列表数据就绪后的公共后处理（SDK 页默认选中第一个 + 项目 pins 下拉补建）
    fn on_sdk_rows_ready(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // SDK 管理页默认选中第一个 SDK，右侧详情直接有内容
        if self.sdk_page.selected.is_none()
            && let Some(rows) = &self.sdk_rows
            && !rows.is_empty()
        {
            self.sdk_page.selected = Some(0);
        }
        // 项目 pins 可能先于 SDK 列表到达：列表就绪后补建下拉
        if self.projects.dir.is_some() && self.projects.selects.is_empty() {
            let rows = self.sdk_rows.clone().unwrap_or_default();
            let none = self.lang.projects.pin_none;
            self.projects.rebuild(&rows, none, window, cx);
        }
    }

    /// 页面导航（保留各页状态）
    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.active = page;
        // 概览页每次进入都重拉：SizeCache 缓存优先（命中毫秒级），
        // 保证安装/卸载/切换后的存储占用与状态列不陈旧
        if page == Page::Overview {
            self.refresh_overview();
        }
        // SDK 管理页进入时若无数据则拉取
        if page == Page::Sdk && self.sdk_rows.is_none() {
            spawn_task(self.tx.clone(), TaskId::SdkListRefresh);
        }
        if page == Page::Projects && self.projects.dir.is_none() {
            // 页面内已有缓存目录则保留；否则从当前工作目录向上找 .sdkm.toml
            spawn_task(self.tx.clone(), TaskId::ProjectAutoLoad);
        }
        if page == Page::Diagnostics && self.diagnostics.env.is_none() {
            self.diagnostics.checking = true;
            spawn_task(self.tx.clone(), TaskId::DiagEnv);
            spawn_task(self.tx.clone(), TaskId::ShellStatus);
        }
        if page == Page::Settings && !self.settings.loaded {
            spawn_task(self.tx.clone(), TaskId::ConfigLoad);
            spawn_task(self.tx.clone(), TaskId::SdkSourcesLoad);
        }
        cx.notify();
    }

    /// 概览刷新统一入口：置 loading 态 + 后台重拉（行数据 + 存储统计）
    fn refresh_overview(&mut self) {
        // 已有刷新在途：只登记补刷，避免并发全量扫描互相覆盖旧结果
        if self.overview.refreshing {
            self.overview.refresh_pending = true;
            return;
        }
        self.overview.refreshing = true;
        self.overview.sizes_loading = true;
        spawn_task(self.tx.clone(), TaskId::OverviewRefresh);
    }

    /// 概览刷新结束后补跑在途期间登记的刷新（多次请求合并为一次）
    fn flush_overview_pending(&mut self) {
        if self.overview.refresh_pending {
            self.overview.refresh_pending = false;
            self.refresh_overview();
        }
    }

    /// 顶部居中轻提示（自动消失；多条按顺序垂直堆叠）
    pub fn notify_error(&mut self, msg: impl Into<String>, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_toast(ToastKind::Error, msg.into(), cx);
    }

    /// 顶部居中警告提示（自动消失；多条按顺序垂直堆叠）
    pub fn notify_warning(&mut self, msg: impl Into<String>, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_toast(ToastKind::Warning, msg.into(), cx);
    }

    /// 顶部居中成功提示（自动消失；多条按顺序垂直堆叠）
    pub fn notify_success(&mut self, msg: impl Into<String>, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_toast(ToastKind::Success, msg.into(), cx);
    }

    fn push_toast(&mut self, kind: ToastKind, msg: String, cx: &mut Context<Self>) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(Toast { id, kind, msg });
        // 最多保留 5 条，避免堆满屏幕（新的追加在下方）
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
        // 4 秒后自动移除该条
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
            let _ = this.update(cx, |app, cx| {
                app.toasts.retain(|t| t.id != id);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 顶部居中提示层（绝对定位，不参与主布局）
    fn render_toasts(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        if self.toasts.is_empty() {
            return None;
        }
        let theme = cx.theme();
        let tks = crate::theme::tokens(cx);
        let list = self.toasts.iter().fold(v_flex().gap_2().items_center(), |acc, toast| {
            let (fg, bg, icon) = match toast.kind {
                ToastKind::Error => (tks.danger_fg, tks.danger_bg, IconName::CircleAlert),
                ToastKind::Warning => (theme.warning, theme.popover, IconName::CircleAlert),
                ToastKind::Success => (tks.success_fg, tks.success_bg, IconName::CircleCheck),
            };
            acc.child(
                h_flex()
                    .id(ElementId::NamedInteger("toast".into(), toast.id))
                    .gap_2()
                    // 图标与文字垂直居中对齐（items_start 会让图标相对文字偏上）
                    .items_center()
                    .min_w(px(260.))
                    .max_w(px(560.))
                    .min_w_0()
                    .overflow_hidden()
                    .px_4()
                    .py_2p5()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(fg)
                    .bg(bg)
                    .child(Icon::new(icon).flex_shrink_0().text_color(fg))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(toast.msg.clone()),
                    ),
            )
        });
        Some(
            div()
                .absolute()
                .top(px(48.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(list),
        )
    }

    /// 切换语言（偏好持久化）
    pub fn set_lang(&mut self, lang: GuiLang, cx: &mut Context<Self>) {
        self.prefs.lang = lang;
        self.lang = Lang::pick(lang);
        self.prefs.save();
        cx.notify();
    }

    /// 设置默认启动页面（偏好持久化）
    pub fn set_default_page(&mut self, page: &str, cx: &mut Context<Self>) {
        self.prefs.default_page = page.to_string();
        self.prefs.save();
        cx.notify();
    }

    /// 切换主题（跟随系统 / 浅色 / 深色，偏好持久化）
    pub fn set_theme(&mut self, mode: GuiThemeMode, cx: &mut Context<Self>) {
        self.prefs.theme_mode = mode;
        crate::theme::apply(mode, cx);
        self.prefs.save();
        cx.notify();
    }

    /// 模糊版本确认弹窗（窗口居中，统一确认样式）
    pub fn open_fuzzy_confirm_dialog(
        &self,
        name: String,
        full_version: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = format!("{} {name} → {full_version}", self.lang.sdk.fuzzy_confirm_title);
        let desc = self.lang.sdk.uninstall_confirm_desc;
        let ok_text = self.lang.common.btn_ok;
        let cancel_text = self.lang.common.btn_cancel;
        let view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(title.clone())
                .description(desc)
                .confirm()
                .ok_text(ok_text)
                .cancel_text(cancel_text)
                .on_ok({
                    let view = view.clone();
                    let name = name.clone();
                    let full_version = full_version.clone();
                    move |_, window, cx| {
                        view.update(cx, |app, cx| {
                            app.dispatch(
                                TaskId::SdkInstallExec {
                                    name: name.clone(),
                                    version: full_version.clone(),
                                },
                                cx,
                            );
                        });
                        // 进度弹窗延后到确认框退出后：on_ok 里直接开会被确认框的延迟关闭连带关掉
                        let deferred = view.clone();
                        let d_name = name.clone();
                        let d_version = full_version.clone();
                        window.defer(cx, move |window, cx| {
                            deferred.update(cx, |app, cx| {
                                // 瞬时操作可能已完成：已完成不再弹窗
                                if !app.sdk_page.busy {
                                    return;
                                }
                                // 标题带上具体 SDK 和解析后的版本号
                                let title = format!("{} {d_name} {d_version}", app.lang.sdk.install_running);
                                let processing = app.lang.sdk.processing;
                                app.open_progress_dialog(title, processing, window, cx);
                            });
                        });
                        true
                    }
                })
        });
    }

    /// 进度弹窗（安装/更新/卸载/导入等长任务；内容实时读 app.progress，Done/Failed 时关闭）
    ///
    /// `title` 为弹窗标题（各操作传入，如"正在卸载 java 21.0.5"）；
    /// `default_phase` 是无进度事件时的阶段文案（有 kind 时按场景覆盖）
    pub fn open_progress_dialog(
        &mut self,
        title: impl Into<SharedString>,
        default_phase: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title: SharedString = title.into();
        // 新操作：清空上一次的单行反馈，并标记弹窗已打开（过程事件分流用）
        self.progress_note = None;
        self.progress_dialog_open = true;
        let view = cx.entity();
        {
            let cancel_view = view.clone();
            window.open_dialog(cx, move |dialog, _, cx: &mut App| {
                let t = view.read(cx).lang;
                let title = title.clone();
                let progress = view.read(cx).progress.clone();
                let kind = progress.as_ref().map(|p| p.kind);
                let pct = progress
                    .as_ref()
                    .map(|p| {
                        if p.total > 0 {
                            (p.pos as f32 / p.total as f32).clamp(0., 1.)
                        } else {
                            0.
                        }
                    })
                    .unwrap_or(0.);
                let phase = match kind {
                    Some(sdkm_core::report::ProgressKind::Download) => t.sdk.progress_download,
                    Some(sdkm_core::report::ProgressKind::ExtractZip)
                    | Some(sdkm_core::report::ProgressKind::ExtractTarGz) => t.sdk.progress_extract,
                    Some(sdkm_core::report::ProgressKind::Resolve) => t.sdk.progress_resolve,
                    Some(sdkm_core::report::ProgressKind::Verify) => t.sdk.progress_verify,
                    Some(sdkm_core::report::ProgressKind::SelfUpdateDownload) => t.sdk.progress_self_update_download,
                    Some(sdkm_core::report::ProgressKind::SelfUpdateExtract) => t.sdk.progress_self_extract,
                    None => default_phase,
                };
                let cancel_view = cancel_view.clone();
                let st = progress.as_ref();
                let has_total = st.is_some_and(|p| p.total > 0);
                // 详情行：下载显示「已下载 / 总大小 · 速度 · 预估剩余」，解压显示条目计数
                let detail = st.and_then(|p| {
                    if matches!(
                        p.kind,
                        sdkm_core::report::ProgressKind::Download | sdkm_core::report::ProgressKind::SelfUpdateDownload
                    ) && has_total
                    {
                        let mut parts = vec![format!(
                            "{} / {}",
                            crate::components::fmt_bytes(p.pos),
                            crate::components::fmt_bytes(p.total)
                        )];
                        if p.speed > 0. {
                            parts.push(format!("{}/s", crate::components::fmt_bytes(p.speed as u64)));
                        }
                        if let Some(s) = p.eta_secs() {
                            let eta = if s >= 60 {
                                format!("{}m{}s", s / 60, s % 60)
                            } else {
                                format!("{s}s")
                            };
                            parts.push(format!("{} {eta}", t.sdk.progress_eta));
                        }
                        Some(parts.join(" · "))
                    } else if has_total {
                        Some(format!("{} / {}", p.pos, p.total))
                    } else {
                        None
                    }
                });
                let bar_color = crate::theme::tokens(cx).primary_soft_fg;
                // 单行过程反馈：core 原文最新一条（不 append，只保留最新）
                let feedback = view.read(cx).progress_note.clone();
                // 有总量：精确百分比进度条（主题蓝）+ 大小/速度/剩余时长；
                // 无总量（解析/校验等）：Spinner + 阶段文案，不显示闪烁进度条
                let content: AnyElement = if has_total {
                    v_flex()
                        .gap_2()
                        .p_1()
                        .child(
                            h_flex().justify_between().child(div().text_sm().child(phase)).child(
                                div()
                                    .text_sm()
                                    .text_color(gpui_kit::rgb(0x64748b))
                                    .child(format!("{:.0}%", pct * 100.)),
                            ),
                        )
                        .child(Progress::new("task-progress").value(pct * 100.).color(bar_color))
                        .when_some(detail, |el, d| {
                            el.child(div().text_xs().text_color(gpui_kit::rgb(0x64748b)).child(d))
                        })
                        .into_any_element()
                } else {
                    h_flex()
                        .gap_2()
                        .p_1()
                        .items_center()
                        .child(Spinner::new().small())
                        .child(div().text_sm().child(phase))
                        .into_any_element()
                };
                // 反馈行固定在进度内容上方：只展示最新一条，过长截断 + hover 看全文
                let content: AnyElement = v_flex()
                    .when_some(feedback, |el, fb| {
                        let tip = fb.clone();
                        el.child(
                            div()
                                .id("progress-feedback")
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(gpui_kit::rgb(0x64748b))
                                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                                .child(fb),
                        )
                    })
                    .child(content)
                    .into_any_element();
                dialog
                    .on_close(move |_, _, cx| {
                        // 用户关闭进度弹窗 = 取消安装：置取消标志并立即恢复页面
                        cancel_view.update(cx, |app, cx| {
                            app.progress_dialog_open = false;
                            let cancelling = app.install_cancel.take();
                            if let Some(token) = &cancelling {
                                token.store(true, Ordering::SeqCst);
                            }
                            // 网络段即时取消：abort 让 future 在下一个 await 点终止；
                            // 解压等阻塞段仍靠上面的标志位在检查点退出
                            if let Some(handle) = app.install_abort.take() {
                                handle.abort();
                            }
                            if cancelling.is_some() {
                                app.sdk_page.busy = false;
                                cx.notify();
                            }
                        });
                    })
                    // 点背景/按 Esc 不关闭：只有显式关闭按钮才取消操作
                    .overlay_closable(false)
                    .keyboard(false)
                    .title(title)
                    .child(content)
            });
        }
    }

    /// 关闭进度弹窗（操作 Done 时）
    pub fn close_progress_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.progress_dialog_open = false;
        window.close_dialog(cx);
    }

    /// 派发操作类任务（SDK 页切换/卸载/安装），置忙碌态
    ///
    /// 网络解析（Resolve）只置 `resolving`（不禁用其他操作）；真正写入（切换/卸载/安装）
    /// 才置 `busy`。安装 Exec 支持关闭进度弹窗取消。
    pub fn dispatch(&mut self, id: TaskId, cx: &mut Context<Self>) {
        if let TaskId::SdkRemoteList { ref name } = id {
            self.last_remote = Some(name.clone());
        }
        match id {
            TaskId::SdkInstallExec { .. } => {
                let cancel = Arc::new(AtomicBool::new(false));
                self.install_cancel = Some(cancel.clone());
                self.install_abort = Some(crate::data::spawn_task_with_cancel(self.tx.clone(), id, cancel));
                self.sdk_page.busy = true;
                self.sdk_page.resolving = false;
            }
            TaskId::SdkInstallResolve { .. } => {
                self.sdk_page.resolving = true;
                spawn_task(self.tx.clone(), id);
            }
            TaskId::SdkSwitch { .. } | TaskId::SdkUninstall { .. } | TaskId::SdkImportLocal { .. } => {
                self.sdk_page.busy = true;
                spawn_task(self.tx.clone(), id);
            }
            other => spawn_task(self.tx.clone(), other),
        }
        cx.notify();
    }

    /// 操作类任务收尾（成功/失败共用）：关进度弹窗 + 解除忙碌态 + 刷新受影响的列表
    fn finish_operation(&mut self, id: &TaskId, window: &mut Window, cx: &mut Context<Self>) {
        // 只读/打开类任务不持有忙碌态：不能因为它们的 Done/Failed 去重置
        // resolving/busy 或关闭正在进行的进度弹窗（否则可发起第二次并发安装）
        if matches!(
            id,
            TaskId::OpenFile { .. }
                | TaskId::OpenConfigPath
                | TaskId::ConfigLoad
                | TaskId::ConfigReload
                | TaskId::SdkSourcesLoad
                | TaskId::SdkSourcesReload
        ) {
            return;
        }
        // 安装完成/取消：先清取消标志，再关弹窗（避免 on_close 误判为用户取消）
        self.install_cancel = None;
        self.install_abort = None;
        self.sdk_page.resolving = false;
        self.sdk_page.import_picking = false;
        self.close_progress_dialog(window, cx);
        self.sdk_page.busy = false;
        self.progress_note = None;
        self.projects.picking = false;
        // 仅 SDK 状态真正变化的任务才刷新列表/概览；设置页等其他任务不做无意义全量扫描
        let affects_sdks = matches!(
            id,
            TaskId::SdkSwitch { .. }
                | TaskId::SdkUninstall { .. }
                | TaskId::SdkInstallExec { .. }
                | TaskId::SdkImportLocal { .. }
                | TaskId::ConfigAddSdk { .. }
                | TaskId::ConfigRemoveSdk { .. }
                | TaskId::ConfigSaveSymlink { .. }
        );
        if affects_sdks {
            spawn_task(self.tx.clone(), TaskId::SdkListRefresh);
            // 安装/卸载/切换/导入/SDK 注册变化都会影响概览（存储占用、当前版本、状态列），同步刷新
            self.refresh_overview();
            // 远程列表的「已安装/使用中」标记同样需要重拉
            if let Some(name) = self.last_remote.clone() {
                spawn_task(self.tx.clone(), TaskId::SdkRemoteList { name });
            }
        }
    }

    /// 操作成功提示文案（仅用户可感知的状态变化返回 Some；None = 不提示）
    fn success_message(&self, id: &TaskId) -> Option<String> {
        let s = &self.lang.sdk;
        let with = |template: &str, pairs: &[(&str, &str)]| -> String {
            let mut out = template.to_string();
            for (key, value) in pairs {
                out = out.replace(key, value);
            }
            out
        };
        match id {
            TaskId::SdkSwitch { name, version } => {
                Some(with(s.toast_switched, &[("{name}", name), ("{version}", version)]))
            }
            TaskId::SdkUninstall { name, version } => {
                Some(with(s.toast_uninstalled, &[("{name}", name), ("{version}", version)]))
            }
            TaskId::SdkInstallExec { name, version } => {
                Some(with(s.toast_installed, &[("{name}", name), ("{version}", version)]))
            }
            TaskId::SdkImportLocal { name, version, .. } => {
                Some(with(s.toast_imported, &[("{name}", name), ("{version}", version)]))
            }
            TaskId::ConfigAddSdk { name, .. } => Some(with(s.toast_sdk_added, &[("{name}", name)])),
            TaskId::ConfigRemoveSdk { name } => Some(with(s.toast_sdk_removed, &[("{name}", name)])),
            // 设置页保存类操作：写入 config.toml 后明确提示
            TaskId::ConfigSaveNetwork { .. } => Some(self.lang.settings.toast_network_saved.to_string()),
            // 恢复内置默认：写明重置范围，让用户知道刚才改了什么
            TaskId::ConfigResetNetwork => Some(self.lang.settings.toast_reset_network.to_string()),
            TaskId::ConfigResetAllSdks => Some(self.lang.settings.toast_reset_all_sdks.to_string()),
            TaskId::ConfigResetSdk { name } => Some(with(self.lang.settings.toast_reset_sdk, &[("{name}", name)])),
            TaskId::SdkSourcesSaveFull { .. } => Some(self.lang.settings.toast_sources_saved.to_string()),
            _ => None,
        }
    }

    /// 重启应用到磁盘上的新版本：往启动时的原始路径拉起新进程后退出当前进程
    /// （不用 current_exe：Linux 上自更新替换后它指向 .discard 副本，spawn 会 ENOENT）
    pub fn restart_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.exe_path.clone().or_else(|| env::current_exe().ok());
        let spawned = match target {
            Some(exe) => Command::new(exe).spawn().map(|_| ()),
            None => Err(std::io::Error::other("cannot locate sdkm-gui executable")),
        };
        match spawned {
            Ok(()) => cx.quit(),
            Err(e) => self.notify_error(format!("restart failed: {e}"), window, cx),
        }
    }

    /// 后台事件分发（window 供弹窗打开/关闭）
    pub fn handle_event(&mut self, ev: GuiEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            GuiEvent::Core(core_ev) => match core_ev {
                CoreEvent::Error(msg) => {
                    // 错误原文直接展示（关键字翻译已移除，避免误报）
                    if self.progress_dialog_open {
                        self.progress_note = Some(msg.clone());
                    }
                    self.notify_error(msg, window, cx);
                }
                // 进度弹窗打开时，过程 warning/detail 原样进弹窗单行反馈
                CoreEvent::Warning(msg) if self.progress_dialog_open => {
                    self.progress_note = Some(msg);
                }
                // 弹窗未打开时 warning 走 toast
                CoreEvent::Warning(msg) => self.notify_warning(msg, window, cx),
                // 过程 Detail（含下载地址）在弹窗打开时原样作为单行反馈
                CoreEvent::Detail(msg) if self.progress_dialog_open => {
                    self.progress_note = Some(msg);
                }
                _ => {}
            },
            // ── 进度事件（操作串行，单槽）──
            GuiEvent::ProgressStart { kind, .. } => {
                self.progress = Some(ProgressState::new(kind));
            }
            GuiEvent::ProgressLength { total, .. } => {
                if let Some(p) = &mut self.progress {
                    p.total = total;
                }
            }
            GuiEvent::ProgressPosition { pos, .. } => {
                if let Some(p) = &mut self.progress {
                    p.sample(pos);
                }
            }
            // 进度文案：进度弹窗打开时作为单行反馈原文展示（重试用）
            GuiEvent::ProgressMessage { msg, .. } => {
                if self.progress_dialog_open {
                    self.progress_note = Some(msg);
                }
            }
            GuiEvent::ProgressFinish { .. } => {
                self.progress = None;
            }
            // ── 任务结果 ──
            GuiEvent::Task {
                id: TaskId::OverviewRefresh,
                outcome: TaskOutcome::Overview(result),
            } => match result {
                Ok(data) => {
                    self.overview.refreshing = false;
                    self.overview.sizes_loading = false;
                    self.overview.total_size = data.total_size;
                    self.overview.init_ok = data.init_ok;
                    self.overview.sizes = data.sizes.into_iter().collect();
                    self.sdk_rows = Some(data.rows);
                    self.on_sdk_rows_ready(window, cx);
                    self.flush_overview_pending();
                }
                Err(e) => {
                    self.overview.refreshing = false;
                    self.overview.refresh_pending = false;
                    self.overview.sizes_loading = false;
                    self.notify_error(e, window, cx)
                }
            },
            GuiEvent::Task {
                id: TaskId::SdkListRefresh,
                outcome: TaskOutcome::SdkList(result),
            } => match result {
                Ok(items) => {
                    self.sdk_rows = Some(items);
                    self.on_sdk_rows_ready(window, cx);
                }
                Err(e) => self.notify_error(e, window, cx),
            },
            GuiEvent::Task {
                id: TaskId::SdkRemoteList { name },
                outcome: TaskOutcome::RemoteList { result, .. },
            } => {
                // 仅当仍是当前选中 SDK 时应用（切换期间的过期结果丢弃）
                let current = self
                    .sdk_page
                    .selected
                    .and_then(|i| self.sdk_rows.as_ref().and_then(|r| r.get(i)))
                    .map(|it| it.name.clone());
                if current.as_deref() == Some(name.as_str()) {
                    self.sdk_page.apply_remote(result, window, cx);
                }
            }
            // 安装解析完成：fuzzy 弹窗确认，精确直接执行
            GuiEvent::Task {
                outcome:
                    TaskOutcome::ResolvedForInstall {
                        name,
                        full_version,
                        fuzzy,
                    },
                ..
            } => {
                self.sdk_page.resolving = false;
                if fuzzy {
                    self.open_fuzzy_confirm_dialog(name, full_version, window, cx);
                } else {
                    // 标题带上具体 SDK 和版本号
                    let title = format!("{} {name} {full_version}", self.lang.sdk.install_running);
                    let processing = self.lang.sdk.processing;
                    self.open_progress_dialog(title, processing, window, cx);
                    self.dispatch(
                        TaskId::SdkInstallExec {
                            name,
                            version: full_version,
                        },
                        cx,
                    );
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::ProjectPins { dir, path, pins },
                ..
            } => {
                let rows = self.sdk_rows.clone().unwrap_or_default();
                let none_label = self.lang.projects.pin_none;
                self.projects.apply_pins(dir, path, pins, &rows, none_label, window, cx);
            }
            GuiEvent::Task {
                outcome: TaskOutcome::DirectoryPicked(picked),
                ..
            } => {
                self.projects.picking = false;
                if let Some(dir) = picked {
                    self.dispatch(TaskId::ProjectLoad { dir }, cx);
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::SymlinkDirPicked(picked),
                ..
            } => {
                self.settings.picking_symlink = false;
                if let Some(dir) = picked {
                    self.dispatch(TaskId::ConfigSaveSymlink { path: dir }, cx);
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::SymlinkSaved(path),
                ..
            } => {
                // 保存成功：回显新目录（空 = 跟随 Home）+ 提示
                self.settings.symlink_dir = path;
                let msg = self.lang.settings.toast_symlink_saved.to_string();
                self.notify_success(msg, window, cx);
            }
            GuiEvent::Task {
                id,
                outcome: TaskOutcome::SdkSources(items),
            } => {
                self.settings.apply_sources(items, window, cx);
                // 仅用户主动点「重新加载」时提示（启动/增删后刷新不打扰）
                if matches!(id, TaskId::SdkSourcesReload) {
                    let msg = self.lang.settings.toast_reloaded.to_string();
                    self.notify_success(msg, window, cx);
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::EnvStatus(result),
                ..
            } => {
                self.diagnostics.checking = false;
                match result {
                    Ok(status) => self.diagnostics.env = Some(status),
                    Err(e) => self.notify_error(e, window, cx),
                }
            }
            // 缓存清理完成：成功给带释放体积的提示
            GuiEvent::Task {
                outcome: TaskOutcome::CleanDone(result),
                ..
            } => {
                self.diagnostics.cleaning = false;
                match result {
                    Ok(bytes) => {
                        let msg = self.lang.diagnostics.clean_done.replace("{size}", &fmt_bytes(bytes));
                        self.notify_success(msg, window, cx);
                    }
                    Err(e) => self.notify_error(e, window, cx),
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::ShellStatus(result),
                ..
            } => {
                self.diagnostics.shell_checking = false;
                self.diagnostics.injecting = false;
                match result {
                    Ok(status) => self.diagnostics.shell = Some(status),
                    Err(e) => self.notify_error(e, window, cx),
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::HookInjected(result),
                ..
            } => {
                self.diagnostics.injecting = false;
                match result {
                    Ok(()) => {
                        // 注入后重新读取 hook 状态
                        spawn_task(self.tx.clone(), TaskId::ShellStatus);
                    }
                    Err(e) => self.notify_error(e, window, cx),
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::ProjectAutoMiss,
                ..
            } => {}
            GuiEvent::Task {
                id,
                outcome:
                    TaskOutcome::ConfigValues {
                        proxy,
                        ssl_verify,
                        timeout,
                        ttl,
                        token,
                        mirror,
                        symlink_dir,
                    },
            } => {
                self.settings
                    .apply_values(proxy, ssl_verify, timeout, ttl, token, mirror, symlink_dir, window, cx);
                // 仅用户主动点「重新加载」时提示（启动时静默回填）
                if matches!(id, TaskId::ConfigReload) {
                    let msg = self.lang.settings.toast_reloaded.to_string();
                    self.notify_success(msg, window, cx);
                }
            }
            GuiEvent::Task {
                outcome: TaskOutcome::LatestVersion(result),
                ..
            } => {
                // 检查更新结果：成功回填最新版本，失败走统一错误条
                self.diagnostics.updating = false;
                match result {
                    Ok(v) => self.diagnostics.latest = Some(v),
                    Err(e) => self.notify_error(e, window, cx),
                }
            }
            // 回退目标查询完成：弹出确认框，展示即将下载的版本号
            GuiEvent::Task {
                outcome: TaskOutcome::PreviousVersion(result),
                ..
            } => {
                self.diagnostics.updating = false;
                if let Ok(v) = result {
                    diagnostics::open_rollback_confirm(self, window, cx, &v);
                }
                // 失败已经由 Error 事件提示
            }
            // 初始化/环境修复完成：关闭进度，刷新数据；首次初始化后进入概览
            GuiEvent::Task {
                outcome: TaskOutcome::InitResult(result),
                ..
            } => {
                self.initializing = false;
                self.diagnostics.repairing = false;
                match result {
                    Ok(()) => {
                        let was_onboarding = self.needs_init;
                        self.needs_init = false;
                        self.init_error = None;
                        if was_onboarding {
                            self.notify_success(self.lang.onboarding.init_done.to_string(), window, cx);
                            // navigate 进入概览页时已触发刷新，无需再调一次
                            self.navigate(Page::Overview, cx);
                        } else {
                            self.notify_success(self.lang.diagnostics.repair_done.to_string(), window, cx);
                            self.diagnostics.env = None;
                            self.diagnostics.shell = None;
                            self.diagnostics.checking = true;
                            self.diagnostics.shell_checking = true;
                            spawn_task(self.tx.clone(), TaskId::DiagEnv);
                            spawn_task(self.tx.clone(), TaskId::ShellStatus);
                        }
                    }
                    Err(e) => {
                        // 引导页保持展示并持久显示错误；诊断页修复失败走轻提示
                        self.init_error = Some(e.clone());
                        self.notify_error(e, window, cx);
                    }
                }
            }
            // 自更新完成：磁盘二进制已替换，但当前进程仍是旧版——
            // 进入"待重启"态：诊断页展示重启入口，版本差异不再按不一致报警
            GuiEvent::Task {
                id: TaskId::UpdateRun | TaskId::UpdateRollback,
                outcome: TaskOutcome::SelfUpdated(outcome),
                ..
            } => {
                self.diagnostics.updating = false;
                self.close_progress_dialog(window, cx);
                match outcome {
                    SelfUpdateOutcome::Updated { version } => {
                        self.restart_required = Some(version);
                        self.notify_warning(self.lang.diagnostics.update_done_restart.to_string(), window, cx)
                    }
                    SelfUpdateOutcome::RolledBack { version } => {
                        self.restart_required = Some(version);
                        self.notify_warning(self.lang.diagnostics.rollback_done_restart.to_string(), window, cx)
                    }
                    SelfUpdateOutcome::UpToDate { version } => {
                        // 磁盘上已是最新但运行进程落后（如在终端里用 CLI 更新过）：同样需要重启
                        if version != env!("CARGO_PKG_VERSION") {
                            self.restart_required = Some(version);
                        }
                    }
                    SelfUpdateOutcome::Checked { .. } => {}
                }
                self.diagnostics.latest = None;
                spawn_task(self.tx.clone(), TaskId::DiagEnv);
            }
            // 自更新失败：错误已由 Error 事件提示，这里只解除忙碌态
            GuiEvent::Task {
                id: TaskId::UpdateRun | TaskId::UpdateRollback,
                outcome: TaskOutcome::UpdateFailed,
                ..
            } => {
                self.diagnostics.updating = false;
                self.close_progress_dialog(window, cx);
                spawn_task(self.tx.clone(), TaskId::DiagEnv);
            }
            // 自定义 SDK 增删后：刷新注册列表 + 源配置 + 概览，并给成功提示
            GuiEvent::Task {
                id,
                outcome: TaskOutcome::Done,
            } if matches!(id, TaskId::ConfigAddSdk { .. } | TaskId::ConfigRemoveSdk { .. }) => {
                spawn_task(self.tx.clone(), TaskId::SdkListRefresh);
                spawn_task(self.tx.clone(), TaskId::SdkSourcesLoad);
                self.refresh_overview();
                if let Some(msg) = self.success_message(&id) {
                    self.notify_success(msg, window, cx);
                }
            }
            // 恢复内置默认：网络重置回填表单；SDK 重置刷新注册列表 / 源草稿 / 概览
            GuiEvent::Task {
                id,
                outcome: TaskOutcome::Done,
            } if matches!(
                id,
                TaskId::ConfigResetNetwork | TaskId::ConfigResetAllSdks | TaskId::ConfigResetSdk { .. }
            ) =>
            {
                if matches!(id, TaskId::ConfigResetNetwork) {
                    spawn_task(self.tx.clone(), TaskId::ConfigReload);
                } else {
                    spawn_task(self.tx.clone(), TaskId::SdkListRefresh);
                    spawn_task(self.tx.clone(), TaskId::SdkSourcesLoad);
                    self.refresh_overview();
                }
                if let Some(msg) = self.success_message(&id) {
                    self.notify_success(msg, window, cx);
                }
            }
            // 导入目录选择完成：选中则弹版本名确认弹窗（取消则清掉锁定的 SDK）
            GuiEvent::Task {
                outcome: TaskOutcome::ImportDirPicked(picked),
                ..
            } => {
                self.sdk_page.import_picking = false;
                if let Some(dir) = picked {
                    sdk::open_import_confirm(self, window, cx, dir);
                } else {
                    self.sdk_page.import_sdk = None;
                }
            }
            // 操作成功：关进度弹窗 + 解除忙碌态 + 刷新列表 + 重拉远程（若开着）+ 成功轻提示
            GuiEvent::Task {
                id,
                outcome: TaskOutcome::Done,
            } => {
                self.finish_operation(&id, window, cx);
                // 成功提示（带 SDK 名/版本，让长时间操作有明确收尾）
                if let Some(msg) = self.success_message(&id) {
                    self.notify_success(msg, window, cx);
                }
            }
            // 操作失败：错误已由 Core Error 事件提示，这里只做与成功路径相同的收尾
            GuiEvent::Task {
                id,
                outcome: TaskOutcome::Failed,
            } => {
                self.finish_operation(&id, window, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    // ── 渲染 ──

    /// 侧边导航（设计图：白色固定栏 + 蓝色方块 logo + 圆角选中项 + 底部版本状态）
    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let t = self.lang;

        let logo = h_flex()
            .gap_2()
            .items_center()
            .px_3()
            .pt_4()
            .pb(px(density().logo_pad_bottom()))
            .child(
                // 品牌应用图标（浅色用彩色版，深色用浅底版；嵌入二进制经自定义 AssetSource 加载）
                // 无文字方形应用图标（固定容器，不随文字挤压/语言切换变化）
                div().size(px(52.)).flex_shrink_0().child(img("app-icon.png").size(px(52.))),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .truncate()
                            .child("sdkm"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(t.common.app_subtitle.to_string()),
                    ),
            );

        let items = [
            (Page::Overview, IconName::House, t.common.nav_overview),
            (Page::Sdk, IconName::Package, t.common.nav_sdk),
            (Page::Projects, IconName::FolderOpen, t.common.nav_projects),
            (Page::Settings, IconName::Settings, t.common.nav_settings),
            (Page::Diagnostics, IconName::ShieldCheck, t.common.nav_diagnostics),
        ];

        let menu = items.iter().fold(v_flex().gap_1().px_2(), |acc, (page, icon, label)| {
            let selected = self.active == *page;
            // 拷贝值进 listener（listener 要求 'static，不能借 items）
            let page = *page;
            let icon = *icon;
            acc.child(
                h_flex()
                    .id(ElementId::Name((*label).into()))
                    .gap_3()
                    .items_center()
                    .px_3()
                    .py(px(density().nav_pad_y()))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(selected, |el| {
                        el.bg(theme.sidebar_accent).text_color(theme.sidebar_accent_foreground)
                    })
                    .when(!selected, |el| {
                        el.text_color(theme.sidebar_foreground).hover(|s| s.bg(theme.muted))
                    })
                    .on_click(cx.listener(move |app, _, _, cx| app.navigate(page, cx)))
                    .child(Icon::new(icon).small())
                    .child(div().text_sm().child((*label).to_string())),
            )
        });

        let footer = v_flex()
            .px_4()
            .pb_4()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("sdkm v{}", self.sys_info.version)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().size_2().rounded_full().bg(if self.overview.init_ok {
                        theme.success
                    } else {
                        theme.warning
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(t.common.running.to_string()),
                    ),
            );

        v_flex()
            .w(px(density().sidebar_width()))
            .h_full()
            .flex_shrink_0()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .justify_between()
            .child(v_flex().child(logo).child(menu))
            .child(footer)
    }

    /// 标题栏（TitleBar：语言/主题下拉 + 窗口控制；不显示页面标题，内容区直接开始）
    fn render_titlebar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let t = self.lang;

        let lang_btn = Button::new("lang-btn")
            .ghost()
            .small()
            .icon(IconName::Globe)
            .label(t.common.lang_name);
        let lang_menu = DropdownButton::new("lang-dropdown")
            .button(lang_btn)
            .dropdown_menu_with_anchor(Anchor::TopRight, |menu, _, _| {
                menu.menu("中文", Box::new(SetLangZh)).menu("English", Box::new(SetLangEn))
            });

        let (theme_label, theme_icon) = match self.prefs.theme_mode {
            GuiThemeMode::System => (t.common.theme_system, IconName::Monitor),
            GuiThemeMode::Light => (t.common.theme_light, IconName::Sun),
            GuiThemeMode::Dark => (t.common.theme_dark, IconName::Moon),
        };
        let theme_menu = DropdownButton::new("theme-dropdown")
            .button(Button::new("theme-btn").ghost().small().icon(theme_icon).label(theme_label))
            .dropdown_menu_with_anchor(Anchor::TopRight, |menu, _, _| {
                menu.menu(t.common.theme_system, Box::new(SetThemeSystem))
                    .menu(t.common.theme_light, Box::new(SetThemeLight))
                    .menu(t.common.theme_dark, Box::new(SetThemeDark))
            });

        gpui_kit::component::TitleBar::new().child(
            // 右侧控件推到窗口按钮之前
            h_flex()
                .gap_1()
                .items_center()
                .justify_end()
                .flex_1()
                .child(lang_menu)
                .child(theme_menu),
        )
    }

    /// 主内容区（可滚动 + 右侧滚动条）
    ///
    /// 设置页自带 tab 锚点滚动容器，需要确定高度，这里不做外层滚动。
    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let page: AnyElement = match self.active {
            Page::Overview => overview::render(self, cx).into_any_element(),
            Page::Sdk => sdk::render(self, cx).into_any_element(),
            Page::Projects => projects::render(self, cx).into_any_element(),
            Page::Settings => settings::render(self, cx).into_any_element(),
            Page::Diagnostics => diagnostics::render(self, cx).into_any_element(),
        };
        if self.active == Page::Settings {
            div()
                .id("content-fixed")
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .px(px(density().page_pad_x()))
                .py(px(density().page_pad_y()))
                .child(page)
                .into_any_element()
        } else {
            div()
                .id("content")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .px(px(density().page_pad_x()))
                .py(px(density().page_pad_y()))
                .child(page)
                .into_any_element()
        }
    }
}

/// 事件泵主体：recv → 分发；窗口关闭（entity 释放）后退出
async fn pump_events(this: WeakEntity<GuiApp>, mut rx: UnboundedReceiver<GuiEvent>, cx: &mut AsyncApp) {
    while let Some(ev) = rx.recv().await {
        // update_in 带窗口上下文，弹窗可在事件处理里直接开/关
        let Ok(()) = this.update_in(cx, |app, window, cx| app.handle_event(ev, window, cx)) else {
            break;
        };
    }
}

impl gpui_kit::Render for GuiApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let root = h_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // 右上角下拉菜单的动作冒泡到根视图统一处理
            .on_action(cx.listener(|app, _: &SetLangZh, _, cx| {
                app.set_lang(GuiLang::Zh, cx);
            }))
            .on_action(cx.listener(|app, _: &SetLangEn, _, cx| {
                app.set_lang(GuiLang::En, cx);
            }))
            .on_action(cx.listener(|app, _: &SetThemeSystem, _, cx| {
                app.set_theme(GuiThemeMode::System, cx);
            }))
            .on_action(cx.listener(|app, _: &SetThemeLight, _, cx| {
                app.set_theme(GuiThemeMode::Light, cx);
            }))
            .on_action(cx.listener(|app, _: &SetThemeDark, _, cx| {
                app.set_theme(GuiThemeMode::Dark, cx);
            }))
            .on_action(cx.listener(|app, _: &SetDefaultOverview, _, cx| {
                app.set_default_page("overview", cx);
            }))
            .on_action(cx.listener(|app, _: &SetDefaultSdk, _, cx| {
                app.set_default_page("sdk", cx);
            }))
            .on_action(cx.listener(|app, _: &SetDefaultProjects, _, cx| {
                app.set_default_page("projects", cx);
            }))
            .on_action(cx.listener(|app, _: &SetDefaultSettings, _, cx| {
                app.set_default_page("settings", cx);
            }))
            .on_action(cx.listener(|app, _: &SetDefaultDiagnostics, _, cx| {
                app.set_default_page("diagnostics", cx);
            }))
            .on_action(cx.listener(|app, _: &sdk::SubmitInstallDialog, _, cx| {
                // 安装弹窗确认：读输入框版本 → 派发解析（选中 SDK）
                let version = app.sdk_page.install_input.read(cx).value().trim().to_string();
                if version.is_empty() {
                    return;
                }
                let Some(name) = app
                    .sdk_page
                    .selected
                    .and_then(|i| app.sdk_rows.as_ref().and_then(|r| r.get(i)))
                    .map(|it| it.name.clone())
                else {
                    return;
                };
                app.dispatch(crate::data::TaskId::SdkInstallResolve { name, version }, cx);
            }))
            .when(!self.needs_init, |el| el.child(self.render_sidebar(cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(self.render_titlebar(cx))
                    .child(if self.needs_init {
                        onboarding::render(self, cx).into_any_element()
                    } else {
                        self.render_content(cx).into_any_element()
                    }),
            )
            .into_any_element();
        div().relative().size_full().child(root).children(self.render_toasts(cx))
    }
}
