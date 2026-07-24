use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct SwitchHandler {
    /// SDK name. Built-in: java, node, python, maven, go; custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven, go. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version (fuzzy match supported, e.g. '21' resolves to latest 21.x; must be installed locally)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x; must be installed locally)"
    )]
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
