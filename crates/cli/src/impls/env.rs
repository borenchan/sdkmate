use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;
use std::env;
use util::shell::{detect_shell, parse_shell};

/// `sdkm env [shell]`：输出当前目录应 eval 的环境变量设置脚本（hook 高频调用）
///
/// **stdout 只吐脚本**（会被 shell eval），禁用 info!/warning! 等 stdout 宏；
/// 诊断（未装降级提示等）在 sdkcore 侧已走 stderr。
#[derive(Debug, Parser)]
pub struct EnvHandler {
    /// Target shell (bash/zsh/fish/powershell); omit to auto-detect
    #[arg(
        long = "shell",
        value_name = "shell",
        help = "Target shell: bash, zsh, fish, powershell (omit to auto-detect)"
    )]
    shell: Option<String>,
}

impl CommandHandler for EnvHandler {
    fn run(&self) -> anyhow::Result<()> {
        let shell = match &self.shell {
            Some(s) => parse_shell(s)?,
            None => detect_shell(),
        };
        let pwd = env::current_dir()?;
        let manager = SdkManager::new()?;
        // 纯脚本输出（缓存命中零解析；无项目配置输出幂等 PATH 重建 + unset 行）
        print!("{}", manager.generate_env_script_cached(shell, &pwd));
        Ok(())
    }
}
