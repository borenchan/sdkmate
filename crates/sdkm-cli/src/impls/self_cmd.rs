use crate::CommandHandler;
use crate::terminal::prompt_confirm;
use crate::terminal_sink::manager;
use clap::{Parser, Subcommand};

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

    /// Update sdkm (CLI + GUI) to the latest GitHub release
    #[command(
        name = "update",
        visible_alias = "u",
        about = "Update sdkm (CLI + GUI) to the latest GitHub release",
        long_about = "Update sdkm to the latest GitHub release.\n\n\
            When sdkm-gui is installed next to sdkm, both binaries are updated together from \
            the desktop bundle so their versions always stay aligned. The new binaries are \
            verified with --version and restored on failure.\n\n\
            --check only reports available updates. --rollback downloads the newest older \
            release that contains the required bundle and replaces the binaries the same way; \
            no local backup is kept."
    )]
    Update(SelfUpdateHandler),
}

impl CommandHandler for SelfHandler {
    fn run(&self) -> anyhow::Result<()> {
        match &self.command {
            SelfSub::Uninstall(h) => h.run(),
            SelfSub::Update(h) => h.run(),
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
            "The sdkm binary itself must be removed manually.\n",
            "Continue?",
        ))?;
        if !confirmed {
            anyhow::bail!("Self-uninstall cancelled by user");
        }
        let mut manager = manager()?;
        manager.uninstall_self()?;
        Ok(())
    }
}

/// 自更新非破坏性（可回退），无需强制确认。
/// `--check`/`-c` 只查询不下载；`--rollback`/`-r` 重新下载上一个版本并替换（与升级同流程）。
/// 二者互斥，同时给则报错。
#[derive(Debug, Parser)]
pub struct SelfUpdateHandler {
    /// Only check for a newer release; do not download or replace
    #[arg(short = 'c', long)]
    check: bool,

    /// Roll back to the previous release (re-download and replace)
    #[arg(short = 'r', long)]
    rollback: bool,
}

impl CommandHandler for SelfUpdateHandler {
    fn run(&self) -> anyhow::Result<()> {
        if self.check && self.rollback {
            anyhow::bail!("`--check` and `--rollback` are mutually exclusive");
        }
        let manager = manager()?;
        manager.update_self(self.check, self.rollback)?;
        Ok(())
    }
}
