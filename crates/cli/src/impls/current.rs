use clap::Parser;
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct CurrentHandler {
    /// The following available SDKs are supported:  java| node | python | rust | maven
    /// Custom SDKs defined in config are also accepted.
    #[arg(value_name = "SDK", help = "Switch the specified SDK to a new version")]
    sdk: Option<String>,
}

impl CommandHandler for CurrentHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        if let Some(sdk_name) = &self.sdk {
            let sdk = manager.match_valid_sdk(sdk_name)?;
            manager.show_local_sdks_current(Some(sdk))?;
        } else {
            manager.show_local_sdks_current(None)?;
        }
        Ok(())
    }
}