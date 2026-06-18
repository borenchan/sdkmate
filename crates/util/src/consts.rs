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
