use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::fmt::Write;
use std::time::Duration;
use util::consts::INSTALL_TIPS;
use util::{info, success};

/// 安装进度条管理器，封装 indicatif 的各阶段样式
pub struct InstallProgress {
    pub pb: ProgressBar,
}

// ── 速度阈值常量（bytes/sec）──
const SPEED_SLOW: f64 = 200_000.0; // < 200 KB/s → 🐢
const SPEED_FAST: f64 = 1_000_000.0; // > 1 MB/s   → 🚀

// ── Tips 轮播周期（秒）──
const TIP_ROTATE_INTERVAL: u64 = 5;

impl InstallProgress {
    /// 创建下载阶段进度条（3 行模板：标题 + 渐变条 + Tips 轮播）
    pub fn new_download(sdk: &str, version: &str) -> Self {
        let pb = ProgressBar::new(0);
        let template = format!(
            "⬇️  {{msg}}\n    {{bar:30.▓▒░.green/blue}} {{percent:>3}}% {{bytes:>10}}/{{total_bytes:>10}} {{speed_icon}}{{bytes_per_sec:>10}} ⏱{{eta:>5}}\n    💡 {{tip}}",
            // first line uses {msg} for title; bar/tip/speed_icon are custom keys
        );

        pb.set_style(
            ProgressStyle::with_template(&template)
                .unwrap()
                .progress_chars("▓▒░")
                .with_key("speed_icon", |state: &ProgressState, w: &mut dyn Write| {
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
                .with_key("tip", |state: &ProgressState, w: &mut dyn Write| {
                    let elapsed_secs = state.elapsed().as_secs();
                    let tip_idx = (elapsed_secs / TIP_ROTATE_INTERVAL) as usize % INSTALL_TIPS.len();
                    w.write_str(INSTALL_TIPS[tip_idx]).unwrap();
                }),
        );
        pb.set_message(format!("Downloading {} {}...", sdk, version));
        pb.enable_steady_tick(Duration::from_millis(500));

        Self { pb }
    }

    /// 创建解压阶段进度条（zip：文件计数 + 百分比）
    pub fn new_extract_zip(sdk: &str, version: &str) -> Self {
        let pb = ProgressBar::new(0);
        let template = format!(
            "📦 {{msg}}\n    {{bar:30.▓▒░.cyan/yellow}} {{percent:>3}}% {{pos:>5}}/{{len:>5}} files ⏱{{eta:>5}}",
        );

        pb.set_style(ProgressStyle::with_template(&template).unwrap().progress_chars("▓▒░"));
        pb.set_message(format!("Extracting {} {}...", sdk, version));
        pb.enable_steady_tick(Duration::from_millis(500));

        Self { pb }
    }

    /// 创建解压阶段 spinner（tar.gz：无法预知文件数，仅动画 + elapsed）
    pub fn new_extract_tar_gz(sdk: &str, version: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        let template = format!("📦 {{msg}} {{spinner:.cyan}} ⏱{{elapsed:>4}}",);

        pb.set_style(
            ProgressStyle::with_template(&template)
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(format!("Extracting {} {}...", sdk, version));
        pb.enable_steady_tick(Duration::from_millis(100));

        Self { pb }
    }

    /// 创建版本解析阶段的 spinner
    pub fn new_resolve(sdk: &str, version_input: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("🔍 {msg} {spinner:.blue}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(format!("Resolving version '{}' for {}...", version_input, sdk));
        pb.enable_steady_tick(Duration::from_millis(100));

        Self { pb }
    }

    /// 创建校验阶段的 spinner
    pub fn new_verify() -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("🔐 {msg} {spinner:.cyan}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message("Verifying installation...");
        pb.enable_steady_tick(Duration::from_millis(100));

        Self { pb }
    }

    /// 完成当前阶段，打印成功消息
    pub fn finish_with_message(&self, msg: String) {
        self.pb.finish_with_message(msg);
    }

    /// 完成当前阶段（清理进度条）
    pub fn finish_and_clear(&self) {
        self.pb.finish_and_clear();
    }

    /// 打印切换阶段提示
    pub fn print_switching(sdk: &str, version: &str) {
        info!("🔗 Switching {} to {}...", sdk, version);
    }

    /// 打印安装成功总结
    pub fn print_success(sdk: &str, version: &str, switched: bool) {
        if switched {
            success!("{} {} installed and switched successfully!", sdk, version);
        } else {
            success!(
                "{} {} installed successfully! (not switched, use 'sdkm switch {} {}' to activate)",
                sdk,
                version,
                sdk,
                version
            );
        }
    }
}
