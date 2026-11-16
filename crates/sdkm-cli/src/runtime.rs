//! CLI 进程级 tokio runtime：core 异步 API 的驱动方（库出 async、二进制持 runtime）
//!
//! 一次性进程用单线程 runtime 足够；OnceLock 惰性创建，首个网络命令才付初始化成本。
//! TUI 交互（prompt/进度条）是同步的，handler 在阶段边界 block_on，不引入 async main。

use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}
