use clap::Parser;
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct SwitchHandler {
    /// The following available SDKs are supported:  java| node | python | rust | maven
    /// Custom SDKs defined in config are also accepted.
    #[arg(value_name = "SDK", help = "Switch the specified SDK to a new version")]
    sdk: String,

    #[arg(help = "the version to switch to")]
    sdk_version: String,

}

impl CommandHandler for SwitchHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        manager.switch_sdk_to_version(&sdk, &self.sdk_version)?;
        Ok(())
    }
}