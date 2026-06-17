use clap::Parser;
use sdkcore::manager::SdkManager;
use crate::CommandHandler;

#[derive(Debug, Parser)]
pub struct InstallHandler {
    /// The following available SDKs are supported: java | node | python | rust | maven
    /// Custom SDKs defined in config are also accepted.
    #[arg(value_name = "SDK", help = "install the specified SDK to a new version")]
    sdk: String,

    /// Target version to install. Supports fuzzy matching like '21' for latest 21.x.
    #[arg(value_name = "VERSION", help = "the target version to install (supports fuzzy matching like '21')")]
    sdk_version: String,

    /// Do not auto-switch to the installed version after installation.
    #[arg(long, help = "install without auto-switching to the new version")]
    no_switch: bool,
}

impl CommandHandler for InstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        let auto_switch = !self.no_switch;
        manager.install_sdk(&sdk, &self.sdk_version, auto_switch)?;
        Ok(())
    }
}
