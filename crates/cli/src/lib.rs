use crate::impls::init::InitHandler;
use crate::impls::list::ListHandler;
use crate::impls::switch::SwitchHandler;
use clap::builder::styling;
use clap::{ColorChoice, Parser, Subcommand};
use crossterm::style::Stylize;
use util::consts::{ABOUT, BANNER, BugReportError};
use util::error;
use util::terminal::suggest_bug_report;
use crate::impls::current::CurrentHandler;
use crate::impls::install::InstallHandler;

mod impls;
mod tui;

#[derive(Debug, Parser)]
#[command(name = "sdkm", author,  version, about = ABOUT.cyan().to_string(), long_about = BANNER.cyan().to_string())]
#[command(propagate_version = true)]  //subcommand extend parent's version
#[command(styles = cargo_style(), color = ColorChoice::Always)] // open color output
pub struct SdkMateCli {

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "init", about = "Initialize sdkm for first-time use")]
    Init(InitHandler),

    #[command(name = "install", visible_alias = "i", about = "Install an SDK version from remote")]
    Install(InstallHandler),

    #[command(name = "list", visible_aliases = ["ls", "l"], about = "List installed or remote SDK versions")]
    List(ListHandler),

    #[command(name = "switch", visible_alias = "s", about = "Switch an SDK to a specific version")]
    Switch(SwitchHandler),

    #[command(name = "current", visible_alias = "c", about = "Show the active version of an SDK")]
    Current(CurrentHandler),

    #[command(name = "config", about = "View or edit sdkm configuration")]
    Config,
}

impl SdkMateCli {
    /// 运行 CLI 应用，返回退出码（0=成功, 1=失败）
    pub fn run(self) -> i32 {
        self.command.run()
    }
}

pub trait CommandHandler {
    /// 执行命令
    ///
    /// 例如打印表格或AscII字符到控制台
    fn run(&self) -> anyhow::Result<()>;
}

impl Commands {
    /// 执行子命令，返回退出码
    pub fn run(self) -> i32 {
        // 获取完整命令行输入（用于 bug report 信息）
        let command_line = full_command_line();
        let res = match self {
            Commands::Init(handler) => handler.run(),
            Commands::Install(handler) => handler.run(),
            Commands::List(handler) => handler.run(),
            Commands::Switch(handler) => handler.run(),
            Commands::Current(handler) => handler.run(),
            _ => Err(anyhow::anyhow!("Not implemented yet")),
        };
        match res {
            Ok(()) => 0,
            Err(cli_err) => {
                error!("{}", cli_err);
                #[cfg(debug_assertions)]
                error!("debug log detail:\n {}", cli_err.backtrace());
                // 检测 BugReport 标记 → 提示 bug report
                if needs_bug_report(&cli_err) {
                    suggest_bug_report(&command_line, &cli_err.to_string());
                }
                1
            }
        }
    }
}

/// 检测 anyhow 错误中是否包含 BugReportError 标记
/// BugReportError 表示该错误不可由用户自行解决，建议提交 bug report
/// 通过 downcast_ref 类型检测，无需字符串匹配
fn needs_bug_report(err: &anyhow::Error) -> bool {
    err.downcast_ref::<BugReportError>().is_some()
}

/// 获取完整命令行输入，如 "switch java 21" 或 "install node 18 --no-switch"
/// 使用 std::env::args 获取原始输入，去掉 args[0]（程序路径）只保留子命令和参数
fn full_command_line() -> String {
    let args: Vec<String> = std::env::args().collect();
    // args[0] 是程序路径（如 "sdkm.exe"），不需要包含在输出中
    args[1..].join(" ")
}

// 定义 cargo 风格的颜色方案
fn cargo_style() -> styling::Styles {
    styling::Styles::styled()
        .header(
            styling::AnsiColor::Green.on_default()
                | styling::Effects::BOLD,
        )
        .usage(
            styling::AnsiColor::Green.on_default()
                | styling::Effects::BOLD,
        )
        .literal(
            styling::AnsiColor::Cyan.on_default()
                | styling::Effects::BOLD,
        )
        .placeholder(styling::AnsiColor::Cyan.on_default())
        .error(
            styling::AnsiColor::Red.on_default()
                | styling::Effects::BOLD,
        )
        .valid(
            styling::AnsiColor::Cyan.on_default()
                | styling::Effects::BOLD,
        )
        .invalid(
            styling::AnsiColor::Yellow.on_default()
                | styling::Effects::BOLD,
        )
}