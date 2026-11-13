// release 构建隐藏控制台窗口（debug 保留日志窗口）
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! sdkm GUI：gpui-kit 桌面前端（独立二进制，复用 sdkcore 业务层）

mod app;
mod assets;
mod components;
mod data;
mod i18n;
mod pages;
mod platform;
mod prefs;
mod sink;
mod theme;

use gpui_kit::*;

fn main() {
    // `sdkm-gui --version`：供自更新替换后校验版本（不打开窗口）
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // GUI 偏好独立于 CLI 配置（gui.toml）
    let prefs = prefs::GuiPrefs::load();

    gpui_kit::application().with_assets(assets::SdkAssets).run(move |cx| {
        // 必须最先调用：初始化组件系统
        gpui_kit::init(cx);

        // 应用设计主题（浅色为设计基准；System 跟随系统外观）
        theme::apply(prefs.theme_mode, cx);

        // Windows 下 icon 字段不赋值，保留 mut 供非 Windows 分支使用
        #[allow(unused_mut)]
        let mut options = WindowOptions {
            // 工具类产品窗口不宜过大；保留弹性收缩的最小尺寸
            window_bounds: Some(WindowBounds::centered(size(px(1240.), px(860.)), cx)),
            window_min_size: Some(size(px(1180.), px(780.))),
            ..component::TitleBar::window_options()
        };
        // 任务栏/窗口标题统一显示 sdkm
        if let Some(titlebar) = &mut options.titlebar {
            titlebar.title = Some("sdkm".into());
        }
        options.app_id = Some("sdkm".into());
        // Windows 的窗口类图标由 exe 资源 ID 1 提供；其余平台用 PNG 解码
        #[cfg(not(windows))]
        {
            options.icon = assets::window_icon();
        }

        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| app::GuiApp::new(window, cx)))
            .expect("打开主窗口失败");
    });
}
