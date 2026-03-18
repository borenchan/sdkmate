use crate::CommandHandler;
use anyhow::Result;
use clap::Parser;
use sdkcore::manager::SdkManager;
use clap::ArgAction::SetTrue;
#[derive(Debug,Parser)]
pub struct InitHandler {
    #[arg(long,short,
            default_value_t = false,
            action=SetTrue,
            help = "force init sdkm, will overwrite the config file"
    )]
    force: bool,

}

impl CommandHandler for InitHandler{
    fn run(&self) -> Result<()> {
        SdkManager::init_sdkm(self.force)?;
        Ok(())
    }
}