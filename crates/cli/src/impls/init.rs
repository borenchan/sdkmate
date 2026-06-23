use crate::CommandHandler;
use anyhow::Result;
use clap::ArgAction::SetTrue;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct InitHandler {
    /// Force reinitialize sdkm (overwrites config, skips dir check)
    #[arg(
        long,
        short,
        default_value_t = false,
        action = SetTrue,
        help = "Force reinitialize sdkm (overwrites config, skips dir check)"
    )]
    force: bool,
}

impl CommandHandler for InitHandler {
    fn run(&self) -> Result<()> {
        SdkManager::init_sdkm(self.force)?;
        Ok(())
    }
}
