use crate::CommandHandler;
use clap::Parser;
use sdkcore::shell::generate_hook_script;
use util::shell::{detect_shell, parse_shell};

/// `sdkm hook [shell]`：输出 shell hook 注册脚本（供 `eval "$(sdkm hook)"` 用）
///
/// **stdout 只吐脚本**（会被 shell eval），禁用 info!/warning! 等 stdout 宏；
/// 诊断走 stderr（error! 宏）。
#[derive(Debug, Parser)]
pub struct HookHandler {
    /// Target shell (bash/zsh/powershell); omit to auto-detect
    #[arg(
        value_name = "shell",
        help = "Target shell: bash, zsh, powershell (omit to auto-detect)"
    )]
    shell: Option<String>,
}

impl CommandHandler for HookHandler {
    fn run(&self) -> anyhow::Result<()> {
        let shell = match &self.shell {
            Some(s) => parse_shell(s)?,
            None => detect_shell(),
        };
        // 纯脚本输出，不加任何人类可读前缀（eval 纯净性）
        print!("{}", generate_hook_script(shell));
        Ok(())
    }
}
