//! 首次启动引导文案

pub struct OnboardingLang {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub step_store: &'static str,
    pub step_path: &'static str,
    pub step_links: &'static str,
    pub step_hook: &'static str,
    pub admin_warning: &'static str,
    pub admin_restart_failed: &'static str,
    pub btn_init: &'static str,
    pub btn_elevate: &'static str,
    pub btn_exit: &'static str,
    pub initializing: &'static str,
    pub init_done: &'static str,
}

/// 简体中文
pub const ZH: OnboardingLang = OnboardingLang {
    title: "初始化 sdkm",
    subtitle: "首次使用需要完成初始化，之后即可通过 GUI 或终端管理 SDK。",
    step_store: "创建配置与存储目录",
    step_path: "将 sdkm 加入系统 PATH",
    step_links: "创建版本链接目录",
    step_hook: "注入 Shell hook（项目级自动切换）",
    admin_warning: "Windows 下初始化需要管理员权限（写入系统 PATH）。请以管理员身份重启 GUI，或右键以管理员身份运行 sdkm-gui。",
    admin_restart_failed: "无法以管理员身份重启，请关闭 GUI 后右键选择“以管理员身份运行”。",
    btn_init: "初始化环境",
    btn_elevate: "以管理员身份重启",
    btn_exit: "退出",
    initializing: "正在初始化，请稍候...",
    init_done: "初始化完成（终端需重新打开后生效）",
};

/// English
pub const EN: OnboardingLang = OnboardingLang {
    title: "Initialize sdkm",
    subtitle: "Finish initialization once before managing SDKs from the GUI or terminal.",
    step_store: "Create config and store directories",
    step_path: "Add sdkm to the system PATH",
    step_links: "Create the version link directory",
    step_hook: "Inject the shell hook (per-project auto switch)",
    admin_warning: "Initialization needs administrator rights on Windows (writes the system PATH). Restart the GUI as administrator, or right-click sdkm-gui and choose Run as administrator.",
    admin_restart_failed: "Could not restart as administrator. Close the GUI and choose Run as administrator.",
    btn_init: "Initialize",
    btn_elevate: "Restart as administrator",
    btn_exit: "Exit",
    initializing: "Initializing, please wait...",
    init_done: "Initialization complete (reopen the terminal to apply PATH changes)",
};
