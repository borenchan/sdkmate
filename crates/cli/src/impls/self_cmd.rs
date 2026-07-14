use crate::CommandHandler;
use clap::{Parser, Subcommand};
use sdkcore::manager::SdkManager;
use util::terminal::prompt_confirm;

/// `sdkm self` 子命令组：管理 sdkm 自身
#[derive(Debug, Parser)]
pub struct SelfHandler {
    #[command(subcommand)]
    command: SelfSub,
}

#[derive(Debug, Subcommand)]
pub enum SelfSub {
    /// Uninstall sdkm itself and clean up all managed SDK environments
    #[command(
        name = "uninstall",
        about = "Uninstall sdkm itself and clean up all managed SDK environments"
    )]
    Uninstall(SelfUninstallHandler),
}

impl CommandHandler for SelfHandler {
    fn run(&self) -> anyhow::Result<()> {
        match &self.command {
            SelfSub::Uninstall(h) => h.run(),
        }
    }
}

/// 自卸载是破坏性操作，必须用户交互确认；CLI 不提供跳过确认的选项。
///
/// 确认逻辑在此层完成，core 的 `uninstall_self` 只做业务（便于测试直接调用）。
#[derive(Debug, Parser)]
pub struct SelfUninstallHandler;

impl CommandHandler for SelfUninstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        // 破坏性操作，强制交互确认，不可跳过
        let confirmed = prompt_confirm(concat!(
            "This will clean up ALL managed SDK environments (symlink/PATH/env/current)\n",
            "and remove the sdkm home directory (store/links/config/cache).\n",
            "The sdkm binary itself and any PATH entry must be removed manually.\n",
            "Continue?",
        ))?;
        if !confirmed {
            anyhow::bail!("Self-uninstall cancelled by user");
        }
        let mut manager = SdkManager::new()?;
        manager.uninstall_self()?;
        Ok(())
    }
}
