use crate::impls::init::InitHandler;
use crate::impls::list::ListHandler;
use crate::impls::switch::SwitchHandler;
use anyhow::Result;
use clap::builder::styling;
use clap::{command, ColorChoice, Parser, Subcommand};
use crossterm::style::Stylize;
use util::consts::{ABOUT, BANNER };
use util::error;
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
    /// 运行 CLI 应用
    pub fn run(self) {
        self.command.run();
    }
}

pub trait CommandHandler {
    /// 执行命令
    ///
    /// 例如打印表格或AscII字符到控制台
    fn run(&self) -> Result<()>;
}
impl Commands {
    /// 执行子命令
    pub fn run(self) {
        let res = match self {
            Commands::Init(handler) => handler.run(),
            Commands::Install(handler) => handler.run(),
            Commands::List(handler) => handler.run(),
            Commands::Switch(handler) => handler.run(),
            Commands::Current(handler) => handler.run(),
            _ => Err(anyhow::anyhow!("Not implemented yet")),
        };
        if let Err (cli_err) = res{
            error!("{}", cli_err);
            #[cfg(debug_assertions)]
            error!("debug log detail:\n {}", cli_err.backtrace());
        }
    }

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