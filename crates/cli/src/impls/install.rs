use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct InstallHandler {
    /// SDK name. Built-in: java, node, python, maven; custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version (fuzzy match supported, e.g. '21' resolves to latest 21.x)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// Do not auto-switch to the installed version
    #[arg(long, help = "Do not auto-switch to the installed version")]
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
