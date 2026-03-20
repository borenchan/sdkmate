use clap::Parser;
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct SwitchHandler {
    #[arg(value_enum, help = "query the list of available versions of a specific SDK")]
    sdk: Sdk,

    #[arg(help = "the version to switch to")]
    sdk_version: String,

}

impl CommandHandler for SwitchHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        manager.switch_sdk_to_version(self.sdk, &self.sdk_version)?;
        Ok(())
    }
}