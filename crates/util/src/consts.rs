use crossterm::style::Stylize;

pub const BANNER: &'static str = r"
     ___  ____  _  _  __  __
    / __||  _ \| |/ /|  \/  |
    \__ \| |_| | ' / | |\/| |
    |___/|____/|_|\_\|_|  |_|
";
pub const ABOUT: &'static str = r"
    SDKM - An SDK version manager for full-stack engineers
    Create By borenchan<boren1007@qq.com>.
";
pub const UNKNOWN: &'static str = "unknown";

pub const SIZE_KB: u64 = 1024;
pub const SIZE_MB: u64 = 1024 * 1024;
pub const SIZE_GB: u64 = SIZE_MB * 1024;

pub const ENV_PATH: &'static str = "PATH";
pub const ENV_JAVA_HOME: &'static str = "JAVA_HOME";

pub const SDKM_STORE_DIR: &'static str = "store";
pub const SDKM_TMP_DIR: &'static str = ".tmp";
pub const CONFIG_FILE_NAME: &'static str = "config.toml";

/// Install 过程中的趣味提示，用于进度条下方轮换展示
pub const INSTALL_TIPS: &[&str] = &[
    "💡 使用 sdkm list <sdk> --source remote 查看所有可用版本",
    "⚡ Rust 驱动：下载速度飞快，内存占用 < 5MB",
    "🧠 输入 '21' 即可模糊匹配最新 LTS 版本",
    "🎯 sdkm 使用符号链接实现毫秒级版本切换",
    "🌍 网络代理可在 config.toml [network] 中配置",
    "🔧 使用 --no-switch 可安装但不自动切换版本",
    "🚀 一条命令搞定安装+切换：sdkm install java 21",
];


// default symlink dir
#[cfg(windows)]
pub const SDKM_SYMLINK_DIR: &'static str = "C:\\Program Files\\sdkm";

#[cfg(unix)]
pub const SDKM_SYMLINK_DIR: &'static str = "/usr/local/sdkm";