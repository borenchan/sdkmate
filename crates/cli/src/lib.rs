use clap::{command, Parser, Subcommand};
use util::consts::BANNER;
use anyhow::Result;
use crossterm::style::Stylize;
use crate::impls::init::InitHandler;
mod impls;

#[derive(Debug, Parser)]
#[command(name = "sdkm", author, version, about, long_about = BANNER )]
pub struct SdkMateCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "init", version, about = "init sdkmate on you first use")]
    Init(InitHandler),

    #[command(name = "install", alias = "i", version, about = "install a new sdk version")]
    Install,

    #[command(name = "list",aliases= ["ls","l"], version, about = "list local sdk versions")]
    List,

    #[command(name = "switch", alias = "s", version, about = "switch sdk version")]
    Switch,

    #[command(name = "current", alias = "c", version, about = "show current active sdk version")]
    Current,

    #[command(name = "config", version, about = "config sdkmate")]
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
            _ => Err(anyhow::anyhow!("Not implemented yet"))
        };
        if let Err (cli_err) = res{
            eprintln!("{}: {}", "error".red().bold(), cli_err.to_string().italic());
        }
    }
}