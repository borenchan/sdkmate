use clap::{command, Parser, Subcommand};
use util::consts::BANNER;
use anyhow::Result;
use crossterm::style::Stylize;
use util::error;
use crate::impls::init::InitHandler;
use crate::impls::list::ListHandler;
use crate::impls::switch::SwitchHandler;

mod impls;

#[derive(Debug, Parser)]
#[command(name = "sdkm", author,  version, about, long_about = BANNER)]
#[command(propagate_version = true)]
pub struct SdkMateCli {

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "init", about = "init sdkmate on you first use")]
    Init(InitHandler),

    #[command(name = "install", alias = "i", about = "install a new sdk version from remote")]
    Install,

    #[command(name = "list",aliases= ["ls","l"], about = "query available sdk and it's versions list")]
    List(ListHandler),

    #[command(name = "switch", alias = "s", about = "switch sdk to another version")]
    Switch(SwitchHandler),

    #[command(name = "current", alias = "c", about = "show current active sdk version")]
    Current,

    #[command(name = "config", about = "config sdkmate")]
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
            Commands::List(handler) => handler.run(),
            Commands::Switch(handler) => handler.run(),
            _ => Err(anyhow::anyhow!("Not implemented yet")),
        };
        if let Err (cli_err) = res{
            error!("{}", cli_err);
            #[cfg(debug_assertions)]
            error!("detail:\n {}", cli_err.backtrace());
        }
    }

}