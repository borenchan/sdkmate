use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;
use util::shell::detect_shell;

/// `sdkm use <SDK> <version> [--shell]`：设置项目级（默认）或会话级版本
///
/// - 默认：写当前目录 `.sdkm.toml`（项目层）
/// - `--shell`：输出会话级 eval 脚本（stdout 只吐脚本，禁 stdout 宏）
#[derive(Debug, Parser)]
pub struct UseHandler {
    /// SDK name. Built-in: java, node, python, maven, go; custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven, go. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version (fuzzy match supported, e.g. '21' resolves to latest 21.x)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// Set for the current shell session only (eval "$(sdkm use --shell java 21)" for bash/zsh; `| source` for fish)
    #[arg(
        long = "shell",
        help = "Set for the current shell session only (bash/zsh: eval \"$(sdkm use --shell <sdk> <version>)\"; fish: sdkm use --shell <sdk> <version> | source)"
    )]
    shell: bool,
}

impl CommandHandler for UseHandler {
    fn run(&self) -> anyhow::Result<()> {
        let manager = SdkManager::new()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        if self.shell {
            // 会话级：stdout 只吐 eval 脚本（禁 stdout 宏）。shell 语法按当前环境自动检测
            let shell = detect_shell();
            let script = manager.use_session_version(shell, &sdk, &self.sdk_version)?;
            print!("{}", script);
        } else {
            // 项目级：交互命令，可用 info!/warning!/success!
            manager.use_project_version(&sdk, &self.sdk_version)?;
        }
        Ok(())
    }
}
