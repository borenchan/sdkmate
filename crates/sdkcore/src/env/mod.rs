use anyhow::Result;
use std::collections::HashMap;

pub trait EnvOperation {
    /// set sdk must require some env variables, it's a optional action
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<()>;

    /// add sdk path to PATH
    fn add_sdk_path(&self, sdk_path: &str) -> Result<()>;

    /// get PATH
    fn get_path(&self) -> Result<String>;

    /// remove a specific path entry from PATH（用于 PATH 冲突清理）
    fn remove_sdk_path(&self, sdk_path: &str) -> Result<()>;

    /// 读取指定环境变量的当前值（用于回滚备份）
    /// 返回 None 表示该变量不存在
    fn get_env_value(&self, key: &str) -> Result<Option<String>>;

    /// 删除指定环境变量（用于回滚 set_sdk_envs）
    /// Windows: 删除注册表值; Unix: 从 shell profile 删除 export 行
    fn unset_sdk_env(&self, key: &str) -> Result<()>;

    /// 批量恢复环境变量到旧值（用于回滚 set_sdk_envs）
    /// 对于有旧值的变量写回旧值，对于没有旧值的变量（None）调用 unset_sdk_env 删除
    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<()>;
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
