//! core 输出契约：结构化事件 + 进度端口（GUI 铺路）
//!
//! 分层铁律：sdkcore 零打印零 stdin，过程消息经 [`EventSink`] 通知前端（CLI 渲染终端 /
//! GUI 渲染界面）；命令结果走返回值。事件只描述"发生了什么"，渲染样式（颜色/图标/
//! 进度条模板/光标操作）全部归前端适配器。
//!
//! 前端接入：
//! - CLI：`cli::terminal_sink::TerminalSink`（sdkm_util::terminal + indicatif 还原终端体验）
//! - GUI：自定义 sink，emit 内转发 channel 到 UI 线程即可获得事件流；耗时操作在专用
//!   后台线程构造并使用 SdkManager——core 公开 API 全同步且内部 block_on，勿在 async
//!   上下文直接调用（嵌套 runtime 会 panic）
//! - 测试：[`NullSink`]（吞事件）

use std::sync::Arc;

/// 过程事件（"发生了什么"），渲染样式归前端
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    Info(String),
    Success(String),
    Warning(String),
    /// 诊断错误（CLI 端走 stderr，其余事件走 stdout）
    Error(String),
    /// 次要细节（CLI 端渲染为缩进灰字）
    Detail(String),
    /// 步骤提示（init 多步流程用）
    Step {
        label: String,
        msg: String,
    },
}

/// 进度场景（前端按 kind 选模板/控件；core 只报场景，不制造虚假精度）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    /// SDK 压缩包下载（bytes 进度，总量已知）
    Download,
    /// sdkm 自身更新下载（bytes 进度，总量已知）
    SelfUpdateDownload,
    /// zip 解压（按文件计数，总数已知）
    ExtractZip,
    /// tar.gz 解压（流式不可计数，时长不确定）
    ExtractTarGz,
    /// 版本解析 / 远程版本列表拉取（时长不确定）
    Resolve,
    /// 安装校验 / 目录归位（时长不确定）
    Verify,
    /// sdkm 自身更新解压（时长不确定）
    SelfUpdateExtract,
}

/// 事件槽：core → 前端的过程通知 + 进度工厂
pub trait EventSink: Send + Sync {
    /// 单条过程事件（下载路径高频调用，实现应轻量）
    fn emit(&self, ev: CoreEvent);

    /// 创建进度句柄；title 是业务文案（core 决定说什么），模板样式前端按 kind 决定
    fn progress(&self, kind: ProgressKind, title: String) -> Box<dyn ProgressHandle>;

    // ── 便捷方法（转发 emit；收 String 以保持 dyn 兼容）──
    fn info(&self, msg: String) {
        self.emit(CoreEvent::Info(msg));
    }
    fn success(&self, msg: String) {
        self.emit(CoreEvent::Success(msg));
    }
    fn warning(&self, msg: String) {
        self.emit(CoreEvent::Warning(msg));
    }
    fn error(&self, msg: String) {
        self.emit(CoreEvent::Error(msg));
    }
    fn detail(&self, msg: String) {
        self.emit(CoreEvent::Detail(msg));
    }
    fn step(&self, label: String, msg: String) {
        self.emit(CoreEvent::Step { label, msg });
    }
}

/// 进度句柄：core 按数据推进，前端渲染
pub trait ProgressHandle: Send {
    fn set_length(&self, total: u64);
    fn set_position(&self, pos: u64);
    fn inc(&self, delta: u64);
    fn set_message(&self, msg: String);
    /// 完成并留下结束语（如 "✅ Resolved ..."）
    fn finish_with_message(&self, msg: String);
    /// 完成并清除进度显示
    fn finish_and_clear(&self);
    /// 前端是否已请求取消（默认 false；GUI 通过共享标志覆盖）
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// 空事件槽：吞掉全部事件，进度 no-op（测试 / 静默场景）
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _ev: CoreEvent) {}
    fn progress(&self, _kind: ProgressKind, _title: String) -> Box<dyn ProgressHandle> {
        Box::new(NullProgress)
    }
}

/// NullSink 配套的空进度句柄
pub struct NullProgress;

impl ProgressHandle for NullProgress {
    fn set_length(&self, _total: u64) {}
    fn set_position(&self, _pos: u64) {}
    fn inc(&self, _delta: u64) {}
    fn set_message(&self, _msg: String) {}
    fn finish_with_message(&self, _msg: String) {}
    fn finish_and_clear(&self) {}
}

/// 便捷构造：Arc 包装的 NullSink
pub fn null_sink() -> Arc<dyn EventSink> {
    Arc::new(NullSink)
}
