use crate::report::{CoreEvent, EventSink};
use anyhow::Result;
use std::collections::HashMap;

/// 环境操作的结构化结果：动作清单（EnvOperation 自身零输出，调用方据此发事件）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvAction {
    /// 设置了环境变量
    EnvSet { key: String, value: String },
    /// 恢复了环境变量旧值（回滚）
    EnvRestored { key: String, value: String },
    /// 目标环境变量不存在，跳过删除
    EnvMissing { key: String },
    /// 删除了环境变量
    EnvRemoved { key: String },
    /// PATH 已含该条目，跳过添加
    PathExists { path: String },
    /// PATH 新增了条目
    PathAdded { path: String },
    /// 从 PATH 移除了条目
    PathRemoved { path: String },
}

impl EnvAction {
    /// 动作 → 过程事件（文案归 core，渲染样式归前端）
    pub fn to_event(&self) -> CoreEvent {
        match self {
            EnvAction::EnvSet { key, value } => {
                CoreEvent::Info(format!("success set env key:`{key}` value:`{value}` !"))
            }
            EnvAction::EnvRestored { key, value } => CoreEvent::Info(format!("restored env `{key}` to `{value}`")),
            EnvAction::EnvMissing { key } => CoreEvent::Warning(format!("env `{key}` does not exist, skip unset")),
            EnvAction::EnvRemoved { key } => CoreEvent::Detail(format!("removed env `{key}`")),
            EnvAction::PathExists { path } => CoreEvent::Warning(format!("path exists. sdk_path: {path}")),
            EnvAction::PathAdded { path } => CoreEvent::Info(format!("success add `{path}` to path!")),
            EnvAction::PathRemoved { path } => CoreEvent::Detail(format!("removed `{path}` from PATH")),
        }
    }
}

/// 把动作清单按序发为事件（manager 调用处统一经此转换，EnvOperation 不感知 UI）
pub fn report_env_actions(sink: &dyn EventSink, actions: &[EnvAction]) {
    for action in actions {
        sink.emit(action.to_event());
    }
}

pub trait EnvOperation {
    /// set sdk must require some env variables, it's a optional action
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<Vec<EnvAction>>;

    /// add sdk path to PATH
    fn add_sdk_path(&self, sdk_path: &str) -> Result<Vec<EnvAction>>;

    /// get PATH
    fn get_path(&self) -> Result<String>;

    /// remove a specific path entry from PATH（用于 PATH 冲突清理）
    fn remove_sdk_path(&self, sdk_path: &str) -> Result<Vec<EnvAction>>;

    /// 读取指定环境变量的当前值（用于回滚备份）
    /// 返回 None 表示该变量不存在
    fn get_env_value(&self, key: &str) -> Result<Option<String>>;

    /// 删除指定环境变量（用于回滚 set_sdk_envs）
    /// Windows: 删除注册表值; Unix: 从 shell profile 删除 export 行
    fn unset_sdk_env(&self, key: &str) -> Result<Vec<EnvAction>>;

    /// 批量恢复环境变量到旧值（用于回滚 set_sdk_envs）
    /// 对于有旧值的变量写回旧值，对于没有旧值的变量（None）调用 unset_sdk_env 删除
    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<Vec<EnvAction>>;
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
