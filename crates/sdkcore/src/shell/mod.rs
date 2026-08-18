//! Shell 集成：hook/env 脚本生成 + shell profile 注入。
//!
//! 分目录收敛 sdkm 的 shell 相关逻辑（此前散在 init.rs / hook_script.rs / env_script.rs）：
//! - `hook.rs`：`sdkm hook <shell>` 输出生成（hook 注册脚本）
//! - `env.rs`：`sdkm env` 输出生成（当前目录 env 设置脚本，高频带缓存）
//! - `inject.rs`：`sdkm init` step 5 的 profile 注入（检测/定位/去重/升级）
//!
//! 底层 Shell 类型与 profile 路径解析在 `util::shell`（通用无依赖逻辑下沉到 util）。

pub mod env;
pub mod hook;
pub mod inject;

/// 生成 hook 脚本（转发出 `hook` 子模块，供 cli 层调用）
pub use hook::generate_hook_script;
