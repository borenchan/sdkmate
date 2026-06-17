use std::collections::HashMap;
use anyhow::Result;
use util::sdk::Sdk;


pub trait EnvOperation {
    /// set sdk must require some env variables, it's a optional action
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<()>;

    /// add sdk path to PATH
    fn add_sdk_path(&self, sdk_path: &str) -> Result<()>;

    /// get PATH
    fn get_path(&self) -> Result<String>;

    /// remove a specific path entry from PATH（用于 PATH 冲突清理）
    fn remove_sdk_path(&self, sdk_path: &str) -> Result<()>;
}

/// PATH 分隔符（Windows 用 ';'，Unix 用 ':'）
pub fn path_separator() -> &'static str {
    if cfg!(target_os = "windows") { ";" } else { ":" }
}

/// 将 PATH 字符串拆分为独立条目
pub fn split_path_entries(path: &str) -> Vec<String> {
    path.split(path_separator())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}


#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsEnvOperation as OsEnvOperation;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::UnixEnvOperation as OsEnvOperation;