use std::error;
use std::fmt;

pub const BANNER: &str = r"
     ___  ____  _  _  __  __
    / __||  _ \| |/ /|  \/  |
    \__ \| |_| | ' / | |\/| |
    |___/|____/|_|\_\|_|  |_|
";
pub const ABOUT: &str = r"
    SDKM - An SDK version manager for full-stack engineers
    Create By borenchan<boren1007@qq.com>.
";
pub const UNKNOWN: &str = "unknown";

pub const SIZE_KB: u64 = 1024;
pub const SIZE_MB: u64 = 1024 * 1024;
pub const SIZE_GB: u64 = SIZE_MB * 1024;

pub const ENV_PATH: &str = "PATH";
pub const ENV_JAVA_HOME: &str = "JAVA_HOME";

pub const SDKM_STORE_DIR: &str = "store";
pub const SDKM_TMP_DIR: &str = ".tmp";
pub const SDKM_CACHE_DIR: &str = ".cache";
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// 项目级配置文件名（项目根 .sdkm.toml，摊平 KV：java = "21"）
pub const PROJECT_CONFIG_FILE_NAME: &str = ".sdkm.toml";

/// shell hook 保存启动 PATH 的变量名（离开项目目录时用它幂等重建 PATH）
pub const ENV_HOOK_BASE_PATH: &str = "_SDKM_BASE_PATH";

/// 会话层版本覆盖的环境变量前缀（SDKM_ACTIVE_JAVA 等，非持久 → 新 shell 丢失）
pub const SDKM_SESSION_ENV_PREFIX: &str = "SDKM_ACTIVE_";

/// init 往 profile 注入 hook 块时的固定注释（inject.rs 写入；unix.rs 插 PATH 行时用它向上回溯到 hook 块之上）
pub const HOOK_COMMENT_LINE: &str = "# sdkm project-level version hook";

/// 目录树中各目录的用途说明
pub const DIR_DESC_STORE: &str = "SDK versions storage";
pub const DIR_DESC_TMP: &str = "download temp (created on install)";
pub const DIR_DESC_CACHE: &str = "version API cache (created on install)";
pub const DIR_DESC_CONFIG: &str = "global config";
pub const DIR_DESC_LINKS: &str = "active SDK symlinks";

/// Visual divider line for terminal output (50 box-drawing chars)
pub const DIVIDER: &str = "──────────────────────────────────────────────────";
/// 软分隔虚点线（同 DIVIDER 宽度；用于 TUI 内分组等弱分隔场景）
pub const DIVIDER_DOT: &str = "· · · · · · · · · · · · · · · · · · · · · · · · ·";

/// Status markers for version display
pub const STATUS_ACTIVE: &str = "✅";
pub const STATUS_INSTALLED: &str = "📦";

/// Tips for install progress bar rotation (English)
pub const INSTALL_TIPS: &[&str] = &[
    "💡 Use sdkm list <sdk> -r to see all available versions",
    "⚡ Rust-powered: fast downloads, < 5MB memory footprint",
    "🧠 Type '21' to fuzzy-match the latest LTS version",
    "🎯 sdkm uses symlinks for millisecond-level version switching",
    "🌍 Network proxy can be configured in config.toml [network]",
    "🔧 Use --no-switch to install without auto-switching",
    "🚀 One command for install+switch: sdkm install java 21",
];

/// Tips for TUI version selector keybindings
pub const TUI_TIPS: &[&str] = &[
    "Press ↑↓ or j/k to navigate, Ctrl+C or q to quit",
    "Press 'i' to install (only available in remote list)",
    "Press Enter or 's' to switch (only installed versions)",
    "Press del/d to uninstall an installed version (with confirm)",
    "✅ active  📦 installed — installed rows highlighted in green",
];

/// Tips for the first-layer SDK selector（已注册 SDK 概览，按键与版本选择器不同）
pub const SDK_SELECTOR_TIPS: &[&str] = &[
    "Press ↑↓ or j/k to navigate, Ctrl+C or q to quit",
    "Press Enter on an installed SDK to browse its local versions",
    "Press r to browse remote versions (Enter on an uninstalled SDK does the same)",
    "✅ active — installed SDKs on top, registered-but-not-installed below",
];

/// 默认符号链接目录名（位于 sdkm home 下，运行时拼成 <home>/links）
pub const SDKM_LINKS_DIR: &str = "links";

/// GitHub issue URL（用于 bug report 提示）
pub const GITHUB_ISSUES_URL: &str = "https://github.com/borenchan/sdkmate/issues/new";

/// 不可由用户解决的错误标记——CLI 层检测到此标记时建议提交 bug report
///
/// 包装内部错误，不修改 Display 输出（用户看到的仍是原始错误消息）。
/// 通过 `downcast_ref::<BugReportError>()` 检测，类型安全无需字符串匹配。
pub struct BugReportError {
    /// 内部错误
    source: anyhow::Error,
}

impl BugReportError {
    /// 包装错误并标记为 bug report
    pub fn wrap(err: anyhow::Error) -> anyhow::Error {
        anyhow::Error::new(Self { source: err })
    }
}

impl fmt::Debug for BugReportError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BugReportError").field("source", &self.source).finish()
    }
}

impl fmt::Display for BugReportError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // 只显示内部错误，不添加额外标记到用户可见的错误消息
        self.source.fmt(f)
    }
}

impl error::Error for BugReportError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
