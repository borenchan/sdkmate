//! 终端事件适配器：把 core 事件/进度还原成现有终端体验
//!
//! CoreEvent 映射 sdkm_util::terminal 语义输出（error 走 stderr，其余 stdout）；进度按
//! ProgressKind 用 indicatif 渲染（模板自原 install/progress.rs 逐字节迁入）。
//! emit 时若有活跃进度条先 suspend 再打印，还原原 downloader 的防交错语义。

use crate::terminal;
use indicatif::{ProgressBar, ProgressStyle};
use sdkm_core::manager::SdkManager;
use sdkm_core::report::{CoreEvent, EventSink, ProgressHandle, ProgressKind};
use sdkm_util::consts::INSTALL_TIPS;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ── 速度阈值常量（bytes/sec）──
const SPEED_SLOW: f64 = 200_000.0; // < 200 KB/s → 🐢
const SPEED_FAST: f64 = 1_000_000.0; // > 1 MB/s   → 🚀

// ── Tips 轮播周期（秒）──
const TIP_ROTATE_INTERVAL: u64 = 5;

/// 终端事件适配器
pub struct TerminalSink {
    /// 活跃进度条槽：emit 前 suspend 防交错（流程串行，同时至多一条）
    active: Arc<Mutex<Option<ProgressBar>>>,
}

impl TerminalSink {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
        }
    }
}

impl EventSink for TerminalSink {
    fn emit(&self, ev: CoreEvent) {
        let guard = self.active.lock().unwrap();
        match guard.as_ref() {
            Some(pb) => pb.suspend(|| render_event(&ev)),
            None => render_event(&ev),
        }
    }

    fn progress(&self, kind: ProgressKind, title: String) -> Box<dyn ProgressHandle> {
        let pb = build_progress(kind, title);
        *self.active.lock().unwrap() = Some(pb.clone());
        Box::new(TermProgress {
            pb,
            slot: Arc::clone(&self.active),
        })
    }
}

/// CLI handler 统一的 manager 构造（显式装配终端适配器）
pub fn manager() -> anyhow::Result<SdkManager> {
    SdkManager::new(Arc::new(TerminalSink::new()))
}

/// 事件 → 终端输出统一入口（与 sdkm_util::terminal 同款调用，样式不变）
fn render_event(ev: &CoreEvent) {
    match ev {
        CoreEvent::Info(m) => terminal::info(m),
        CoreEvent::Success(m) => terminal::success(m),
        CoreEvent::Warning(m) => terminal::warning(m),
        CoreEvent::Error(m) => terminal::error(m),
        CoreEvent::Detail(m) => terminal::detail(m),
        CoreEvent::Step { label, msg } => terminal::step(label, msg),
    }
}

/// 按 kind 构建进度条（模板逐字节迁自原 install/progress.rs 与 self_update.rs，title 由 core 传入）
fn build_progress(kind: ProgressKind, title: String) -> ProgressBar {
    match kind {
        ProgressKind::Download => {
            // no_length：len=None 显示 0%；new(0) 是 len=Some(0)，set_length 前的窗口期 fraction=1.0 会画成 100%
            let pb = ProgressBar::no_length();
            let template = "⬇️ {msg}\n    {bar:30.▓▒░.green/bright_blue} {percent:>3}% {bytes:>10}/{total_bytes:>10} {speed_icon}{bytes_per_sec:>10} ⏱{eta:>5}\n    💡 {tip}";
            pb.set_style(
                ProgressStyle::with_template(template)
                    .unwrap()
                    .progress_chars("▓▒░")
                    .with_key("speed_icon", |state: &indicatif::ProgressState, w: &mut dyn Write| {
                        let speed = state.per_sec();
                        let icon = if speed <= 0.0 || state.elapsed() < Duration::from_secs(1) {
                            "⏳" // 刚启动，速度尚未确定
                        } else if speed < SPEED_SLOW {
                            "🐢" // 慢速
                        } else if speed < SPEED_FAST {
                            "⚡" // 正常
                        } else {
                            "🚀" // 高速
                        };
                        w.write_str(icon).unwrap();
                    })
                    .with_key("tip", |state: &indicatif::ProgressState, w: &mut dyn Write| {
                        let elapsed_secs = state.elapsed().as_secs();
                        let tip_idx = (elapsed_secs / TIP_ROTATE_INTERVAL) as usize % INSTALL_TIPS.len();
                        w.write_str(INSTALL_TIPS[tip_idx]).unwrap();
                    }),
            );
            pb.set_message(title);
            pb.enable_steady_tick(Duration::from_millis(500));
            pb
        }
        ProgressKind::SelfUpdateDownload => {
            // 自更新精简条（原 self_update::download_progress，new(0) 保持原样）
            let pb = ProgressBar::new(0);
            pb.set_style(
                ProgressStyle::with_template("  {msg}\n  {bar:40.green/blue} {bytes}/{total_bytes} ({bytes_per_sec})")
                    .unwrap()
                    .progress_chars("▓▒░"),
            );
            pb.set_message(title);
            pb.enable_steady_tick(Duration::from_millis(500));
            pb
        }
        ProgressKind::ExtractZip => {
            // 同 Download：no_length 避免 set_length 前 len=Some(0) 画成 100%
            // pos/len 不加右对齐 pad：原版 {pos:>5}/{len:>5} 在 3/3 这类小数字下裸出大片空格，用户裁定改掉；
            // % 后固定两空格使 files 段起点对齐下载条 {bytes:>10} 的数字起点（9 字符典型值，用户裁定）
            let pb = ProgressBar::no_length();
            let template = "📦 {msg}\n    {bar:30.▓▒░.cyan/yellow} {percent:>3}%  {pos}/{len} files ⏱{eta:>5}";
            pb.set_style(ProgressStyle::with_template(template).unwrap().progress_chars("▓▒░"));
            pb.set_message(title);
            pb.enable_steady_tick(Duration::from_millis(500));
            pb
        }
        ProgressKind::ExtractTarGz | ProgressKind::Resolve | ProgressKind::Verify => {
            // 三种场景仅模板差异：tar.gz 青转带耗时、resolve 蓝转、verify 青转（均迁自 install/progress.rs）
            let template = match kind {
                ProgressKind::ExtractTarGz => "📦 {msg} {spinner:.cyan} ⏱{elapsed:>4}",
                ProgressKind::Resolve => "🔍 {msg} {spinner:.blue}",
                _ => "🔐 {msg} {spinner:.cyan}",
            };
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template(template)
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
            pb.set_message(title);
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        }
        ProgressKind::SelfUpdateExtract => {
            // 自更新解压 spinner（原 self_update::spinner_progress，无前缀两空格缩进）
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("  {spinner} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
            pb.set_message(title);
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        }
    }
}

/// indicatif 进度句柄实现，finish 时清活跃槽
struct TermProgress {
    pb: ProgressBar,
    slot: Arc<Mutex<Option<ProgressBar>>>,
}

impl Drop for TermProgress {
    /// 兜底清理：core 侧错误路径（`?` 提前返回）不会显式 finish，
    /// Drop 时未结束的进度条清屏收尾并释放活跃槽，避免终端残留；
    /// 已 finish 的 bar 幂等跳过，不与显式 finish 冲突
    fn drop(&mut self) {
        if !self.pb.is_finished() {
            self.pb.finish_and_clear();
        }
        *self.slot.lock().unwrap() = None;
    }
}

impl ProgressHandle for TermProgress {
    fn set_length(&self, total: u64) {
        self.pb.set_length(total);
    }
    fn set_position(&self, pos: u64) {
        self.pb.set_position(pos);
    }
    fn inc(&self, delta: u64) {
        self.pb.inc(delta);
    }
    fn set_message(&self, msg: String) {
        self.pb.set_message(msg);
    }
    fn finish_with_message(&self, msg: String) {
        self.pb.finish_with_message(msg);
        *self.slot.lock().unwrap() = None;
    }
    fn finish_and_clear(&self) {
        self.pb.finish_and_clear();
        *self.slot.lock().unwrap() = None;
    }
}
