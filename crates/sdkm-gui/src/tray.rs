//! 系统托盘：tray-icon（Windows 隐藏窗口 + Shell_NotifyIconW，借 GPUI 主线程消息循环；
//! macOS NSStatusItem；Linux ksni 纯 Rust D-Bus 后端）
//!
//! 事件模型：平台回调经 `set_event_handler` 全局闭包转发进既有 `GuiEvent`
//! 通道（tokio mpsc 发送端线程安全），UI 线程 pump 统一处理，无轮询线程。
//!
//! 窗口最小化到托盘（Windows）：GPUI 未提供窗口 hide/show API，用
//! raw-window-handle 取 HWND 后 `ShowWindowAsync(SW_HIDE/SW_SHOW)`，
//! 隐藏/恢复触发 WM_SHOWWINDOW，gpui 的 visibility 上报照常工作。

use std::sync::Mutex;

use crate::i18n::Lang;
use crate::sink::GuiEvent;
use gpui_kit::Window;
use tokio::sync::mpsc::UnboundedSender;
use tray_icon::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 托盘触发动作（经 GuiEvent 通道转 UI 线程）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    /// 显示/聚焦主窗口
    Show,
    /// 显示主窗口并直达设置页
    Settings,
    /// 重启应用（自更新后生效）
    Restart,
    /// 退出应用
    Quit,
    /// 快速切换已安装版本
    Switch { sdk: String, version: String },
}

/// 右键菜单项 id（事件回传时匹配）
const MENU_ID_OPEN: &str = "sdkm-tray-open";
const MENU_ID_SETTINGS: &str = "sdkm-tray-settings";
const MENU_ID_RESTART: &str = "sdkm-tray-restart";
const MENU_ID_QUIT: &str = "sdkm-tray-quit";
const MENU_ID_VERSION: &str = "sdkm-tray-version";
/// 快速切换项 id 前缀（后接 `sdk|version`）
const MENU_ID_SWITCH: &str = "sdkm-tray-switch|";

/// 快速切换条目：一个有已安装版本的 SDK
pub struct TraySwitchEntry {
    pub sdk: String,
    /// 当前激活版本（打 ✓）
    pub current: Option<String>,
    pub versions: Vec<String>,
}

/// 菜单可变量（语言 / 快速切换条目 / 忙碌态）；任一变化整体重建菜单
struct MenuState {
    lang: &'static Lang,
    entries: Vec<TraySwitchEntry>,
    busy: bool,
}

/// 托盘状态（持有 TrayIcon；Drop 自动移除图标——Windows 发 NIM_DELETE，防残留）
pub struct TrayState {
    /// 托盘句柄本体：仅持有不读，Drop 时移除图标（dead_code 豁免）
    #[allow(dead_code)]
    icon: TrayIcon,
    state: Mutex<MenuState>,
}

impl TrayState {
    /// 同步菜单文案（语言切换时调用；菜单在 UI 线程重建）
    pub fn set_lang(&self, lang: &'static Lang) {
        self.with_state(|s| s.lang = lang);
    }

    /// 操作进行中置灰"退出/重启/快速切换"（避免中途退出留下残缺版本目录）
    pub fn set_busy(&self, busy: bool) {
        self.with_state(|s| s.busy = busy);
    }

    /// 更新快速切换列表（仅已安装版本的 SDK；SDK 列表刷新后调用）
    pub fn set_switch_entries(&self, entries: Vec<TraySwitchEntry>) {
        self.with_state(|s| s.entries = entries);
    }

    /// 改状态并重建菜单（锁内完成，避免并发重建交错）
    fn with_state(&self, f: impl FnOnce(&mut MenuState)) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut state);
        rebuild(&self.icon, &state);
    }
}

/// 创建托盘；失败返回 None（行为退回"关闭即退出"，不影响启动）
///
/// 事件 handler 为进程级全局注册，本应用单窗口只调用一次。
pub fn init(tx: UnboundedSender<GuiEvent>, lang: &'static Lang) -> Option<TrayState> {
    // 左键单击（抬起）→ 显示主窗口；右键由托盘库弹菜单
    let tx_click = tx.clone();
    TrayIconEvent::set_event_handler(Some(move |ev| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = ev
        {
            let _ = tx_click.send(GuiEvent::Tray(TrayCommand::Show));
        }
    }));

    // 菜单项点击 → 按 id 分发动作
    let tx_menu = tx;
    MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
        if let Some(cmd) = command_for(ev.id.0.as_str()) {
            let _ = tx_menu.send(GuiEvent::Tray(cmd));
        }
    }));

    let icon = load_icon()?;
    let icon = TrayIconBuilder::new()
        .with_tooltip("sdkm")
        .with_icon(icon)
        // 左键单击不弹菜单（触发 Show 事件）；菜单固定右键
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .build()
        .ok()?;

    let state = MenuState {
        lang,
        entries: Vec::new(),
        busy: false,
    };
    rebuild(&icon, &state);

    Some(TrayState {
        icon,
        state: Mutex::new(state),
    })
}

/// 菜单 id → 动作（快速切换项携带 sdk + version）
fn command_for(id: &str) -> Option<TrayCommand> {
    match id {
        MENU_ID_OPEN => Some(TrayCommand::Show),
        MENU_ID_SETTINGS => Some(TrayCommand::Settings),
        MENU_ID_RESTART => Some(TrayCommand::Restart),
        MENU_ID_QUIT => Some(TrayCommand::Quit),
        _ => {
            let (sdk, version) = id.strip_prefix(MENU_ID_SWITCH)?.split_once('|')?;
            Some(TrayCommand::Switch {
                sdk: sdk.to_string(),
                version: version.to_string(),
            })
        }
    }
}

/// 按当前状态重建整个菜单（列表/语言/忙碌态变化都走这里，避免增量同步逻辑）
fn rebuild(icon: &TrayIcon, state: &MenuState) {
    let s = &state.lang.common;
    let menu = Menu::new();

    let item_open = MenuItem::with_id(MENU_ID_OPEN, s.tray_open, true, None);
    let item_settings = MenuItem::with_id(MENU_ID_SETTINGS, s.tray_settings, true, None);
    let sep_top = PredefinedMenuItem::separator();

    // 快速切换：每个有已安装版本的 SDK 一个子菜单，当前版本打 ✓；列表为空时整体置灰
    let switch = Submenu::new(s.tray_switch, !state.busy && !state.entries.is_empty());
    for entry in &state.entries {
        let sub = Submenu::new(&entry.sdk, !state.busy);
        for version in &entry.versions {
            let text = if entry.current.as_deref() == Some(version.as_str()) {
                format!("✓ {version}")
            } else {
                version.clone()
            };
            let id = format!("{MENU_ID_SWITCH}{}|{version}", entry.sdk);
            let item = MenuItem::with_id(id, text, !state.busy, None);
            if sub.append(&item).is_err() {
                break;
            }
        }
        if switch.append(&sub).is_err() {
            break;
        }
    }
    let sep_mid = PredefinedMenuItem::separator();

    // 版本展示（禁用项，仅作信息行）
    let item_version = MenuItem::with_id(MENU_ID_VERSION, concat!("sdkm v", env!("CARGO_PKG_VERSION")), false, None);
    let item_restart = MenuItem::with_id(MENU_ID_RESTART, s.tray_restart, !state.busy, None);
    let item_quit = MenuItem::with_id(MENU_ID_QUIT, s.tray_quit, !state.busy, None);

    let items: [&dyn IsMenuItem; 8] = [
        &item_open,
        &item_settings,
        &sep_top,
        &switch,
        &sep_mid,
        &item_version,
        &item_restart,
        &item_quit,
    ];
    if menu.append_items(&items).is_ok() {
        icon.set_menu(Some(Box::new(menu)));
    }
}

/// 托盘图标：内嵌应用图标 PNG → RGBA（gpui 已带 image 依赖，无额外开销）
fn load_icon() -> Option<Icon> {
    let bytes = include_bytes!("../../../assets/gui-icons/02-app-icon-simplified-128.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// 主窗口原始 HWND（Windows 专用）
#[cfg(windows)]
fn raw_hwnd(window: &mut Window) -> Option<*mut core::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut _),
        _ => None,
    }
}

/// 隐藏主窗口到托盘（Windows：SW_HIDE；恢复走 show_window）
#[cfg(windows)]
pub fn hide_window(window: &mut Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindowAsync};
    if let Some(hwnd) = raw_hwnd(window) {
        unsafe { ShowWindowAsync(hwnd, SW_HIDE) };
    }
}

/// 从托盘恢复并聚焦主窗口（Windows 先 SW_SHOW 再 activate——
/// Windows 的 activate 对隐藏窗口不会自动显示；macOS/Linux 窗口未隐藏，直接聚焦）
#[cfg(windows)]
pub fn show_window(window: &mut Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOW, ShowWindowAsync};
    if let Some(hwnd) = raw_hwnd(window) {
        unsafe { ShowWindowAsync(hwnd, SW_SHOW) };
    }
    window.activate_window();
}

#[cfg(not(windows))]
pub fn show_window(window: &mut Window) {
    // 关闭即退出的平台上窗口不会隐藏，托盘"打开"仅聚焦
    window.activate_window();
}
