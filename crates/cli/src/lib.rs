use clap::{Parser, Subcommand};
use crossterm::style::Stylize;

#[derive(Debug, Parser)]
#[command(name = "sdkm", author, version, about, long_about = BANNER )]
pub struct SdkMateCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "init", version, about = "init sdkmate on you first use")]
    Init(CmdHandler),
    #[command(name = "install", alias = "i", version, about = "install a new sdk version")]
    Install(),
    #[command(name = "list",aliases= ["ls","l"], version, about = "list local sdk versions")]
    List(CmdHandler),

    #[command(name = "switch", alias = "s", version, about = "switch sdk version")]
    Switch(CmdHandler),

    #[command(name = "current", alias = "c", version, about = "show current active sdk version")]
    Current(CmdHandler),

    #[command(name = "config", version, about = "config sdkmate")]
    Config(CmdHandler),
}

#[derive(Debug,Parser)]
struct CmdHandler {
    #[arg(required =  true,help = "sdk name",long,short)]
    name: String
}
impl CmdHandler {
    pub fn run(&self) {
        println!("hello: {}",self.name);
    }

}
impl Commands {
    /// 执行子命令
    pub fn run(self) {
        match self {
            (handler) => handler.run(),
        }
        // let combine_handlers = CombineHandler::new();
        // match combine_handlers.matches_handler(self) {
        //     Ok(handler) => {
        //         if let Err(cli_err) = handler.run() {
        //             eprintln!("{}: {}", "error".red().bold(), cli_err.to_string().italic());
        //         }
        //     }
        //     Err(cli_err) => {
        //         eprintln!("{}: {}", "error".red().bold(), cli_err.to_string().italic());
        //     }
        // }
    }
}
