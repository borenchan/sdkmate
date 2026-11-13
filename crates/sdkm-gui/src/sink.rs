//! GUI 事件桥：sdkm_core::report 的 EventSink 实现 → tokio mpsc channel → UI 线程
//!
//! 约定：每个后台操作独立构造 Manager+Sink，第一版操作
//! 串行执行（单通道不区分任务）；进度按 kind 场景转发，UI 端选模板渲染。
//!
//! tokio::sync 的 channel runtime 无关（官方文档明确）：发送端 unbounded_send 是
//! 纯同步方法（后台线程直接调），接收端 recv() 在 gpui 自己的执行器上 await 即可。

use sdkm_core::report::{CoreEvent, EventSink, ProgressHandle, ProgressKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::UnboundedSender;

/// 进度生命周期事件（UI 端按 id 维护活动进度条；进度消费随安装流程接入，字段暂未读取属预期）
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum GuiEvent {
    /// 过程消息（Info/Success/Warning/Error/Detail/Step）
    Core(CoreEvent),
    /// 进度开始（id 自增区分多次操作）
    ProgressStart { id: u64, kind: ProgressKind, title: String },
    /// 进度总量确定（bytes / 文件计数）
    ProgressLength { id: u64, total: u64 },
    /// 进度推进（绝对位置）
    ProgressPosition { id: u64, pos: u64 },
    /// 进度文案更新
    ProgressMessage { id: u64, msg: String },
    /// 进度结束（带结束语或静默清除）
    ProgressFinish { id: u64, msg: Option<String> },
    /// 后台任务结果回传（UI 端按 id 分发到页面状态）
    Task {
        id: crate::data::TaskId,
        outcome: crate::data::TaskOutcome,
    },
}

/// EventSink 实现：全部事件经 channel 转发到 UI 线程（发送失败静默——UI 已关闭）
pub struct GuiSink {
    tx: UnboundedSender<GuiEvent>,
    next_id: std::sync::atomic::AtomicU64,
    /// 取消标志（安装等长任务；None = 不可取消）
    cancel: Option<Arc<AtomicBool>>,
}

impl GuiSink {
    pub fn with_cancel(tx: UnboundedSender<GuiEvent>, cancel: Option<Arc<AtomicBool>>) -> Self {
        Self {
            tx,
            next_id: std::sync::atomic::AtomicU64::new(1),
            cancel,
        }
    }
}

impl EventSink for GuiSink {
    fn emit(&self, ev: CoreEvent) {
        let _ = self.tx.send(GuiEvent::Core(ev));
    }

    fn progress(&self, kind: ProgressKind, title: String) -> Box<dyn ProgressHandle> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = self.tx.send(GuiEvent::ProgressStart { id, kind, title });
        Box::new(GuiProgress {
            tx: self.tx.clone(),
            id,
            pos: std::sync::atomic::AtomicU64::new(0),
            cancel: self.cancel.clone(),
        })
    }
}

/// 进度句柄：core 按数据推进，事件转 UI（Drop 兜底静默清除未 finish 的进度）
struct GuiProgress {
    tx: UnboundedSender<GuiEvent>,
    id: u64,
    /// 本地累加器：inc 场景（解压计数）在句柄内转绝对值，UI 端只处理绝对位置
    pos: std::sync::atomic::AtomicU64,
    cancel: Option<Arc<AtomicBool>>,
}

impl GuiProgress {
    fn send(&self, ev: GuiEvent) {
        let _ = self.tx.send(ev);
    }
}

impl ProgressHandle for GuiProgress {
    fn set_length(&self, total: u64) {
        self.send(GuiEvent::ProgressLength { id: self.id, total });
    }
    fn set_position(&self, pos: u64) {
        self.pos.store(pos, std::sync::atomic::Ordering::Relaxed);
        self.send(GuiEvent::ProgressPosition { id: self.id, pos });
    }
    fn inc(&self, delta: u64) {
        let pos = self.pos.fetch_add(delta, std::sync::atomic::Ordering::Relaxed) + delta;
        self.send(GuiEvent::ProgressPosition { id: self.id, pos });
    }
    fn set_message(&self, msg: String) {
        self.send(GuiEvent::ProgressMessage { id: self.id, msg });
    }
    fn finish_with_message(&self, msg: String) {
        self.send(GuiEvent::ProgressFinish {
            id: self.id,
            msg: Some(msg),
        });
    }
    fn finish_and_clear(&self) {
        self.send(GuiEvent::ProgressFinish { id: self.id, msg: None });
    }
    fn is_cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst))
    }
}

impl Drop for GuiProgress {
    fn drop(&mut self) {
        // Drop 兜底：core 错误路径 ? 提前返回时清理未 finish 的进度展示
        self.send(GuiEvent::ProgressFinish { id: self.id, msg: None });
    }
}
