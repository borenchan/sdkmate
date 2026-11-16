//! core 同步/异步桥接用的 tokio runtime 构造
//!
//! core 的异步流程全是顺序 await，没有 `tokio::spawn` / `join` 并发；
//! 单线程 runtime 足够，且不再为每次调用拉起每核一个 worker 线程
//! （GUI 每个后台任务都会构造一次 runtime）。

use anyhow::{Context, Result};

/// 构造单线程 tokio runtime（启用 IO / 时间驱动）
pub(crate) fn new_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")
}
