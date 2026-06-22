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
pub const SDKM_CACHE_DIR: &str = "cache";
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// 目录树中各目录的用途说明
pub const DIR_DESC_STORE: &str = "SDK versions storage";
pub const DIR_DESC_TMP: &str = "download temp (created on install)";
pub const DIR_DESC_CACHE: &str = "version API cache (created on install)";
pub const DIR_DESC_CONFIG: &str = "global config";

/// Visual divider line for terminal output (50 box-drawing chars)
pub const DIVIDER: &str = "──────────────────────────────────────────────────";

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
    "✅ active  📦 installed — installed rows highlighted in green",
];

// default symlink dir
#[cfg(windows)]
pub const SDKM_SYMLINK_DIR: &str = "C:\\Program Files\\sdkm";

#[cfg(unix)]
pub const SDKM_SYMLINK_DIR: &str = "/usr/local/sdkm";

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

impl std::fmt::Debug for BugReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("BugReportError")
            .field("source", &self.source)
            .finish()
    }
}

impl std::fmt::Display for BugReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // 只显示内部错误，不添加额外标记到用户可见的错误消息
        self.source.fmt(f)
    }
}

impl std::error::Error for BugReportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
