use crate::CommandHandler;
use clap::{Parser, Subcommand};
use sdkcore::manager::SdkManager;

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
/// 内部调用 `uninstall_self(false)`——core 的 `yes` 参数仅作为集成测试逃生口
/// （测试无法应答 stdin），CLI 永远传 `false` 强制确认。
#[derive(Debug, Parser)]
pub struct SelfUninstallHandler;

impl CommandHandler for SelfUninstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        manager.uninstall_self(false)?;
        Ok(())
    }
}
