//! core 阻塞段调度：全项目唯一知道"阻塞任务怎么跑"的地方。
//! core 自身不持有 runtime——异步 API 由前端（CLI/GUI）的常驻 runtime 驱动。

use anyhow::{Context, Result};

/// 把无 await 点的长阻塞段（解压/目录清理/校验）交给专用阻塞线程池执行，
/// 避免占用 runtime worker（共享 runtime 场景会饿死同池任务与定时器）。
/// 全项目唯一知道"阻塞任务怎么跑"的地方，将来更换运行时只需改此文件。
pub(crate) async fn run_blocking<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.context("Blocking task join failed")
}
