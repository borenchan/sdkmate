use clap::Parser;
use sdkcore::manager::SdkManager;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct InstallHandler {
    /// The following available SDKs are supported:  java| node | python | rust | maven
    /// Custom SDKs defined in config are also accepted.
    #[arg(value_name = "SDK", help = "install the specified SDK to a new version")]
    sdk: String,

    #[arg(help = "the target version to install")]
    sdk_version: String,

}

impl CommandHandler for InstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        Ok(())
    }
}