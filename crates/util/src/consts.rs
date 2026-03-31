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


// default symlink dir
#[cfg(windows)]
pub const SDKM_SYMLINK_DIR: &'static str = "C:\\Program Files\\sdkm";

#[cfg(unix)]
pub const SDKM_SYMLINK_DIR: &'static str = "/usr/local/sdkm";