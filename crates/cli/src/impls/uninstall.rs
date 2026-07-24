use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct UninstallHandler {
    /// SDK name. Built-in: java, node, python, maven, go; custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven, go. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version to uninstall (fuzzy match supported, e.g. '21' resolves to latest 21.x)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// Skip all interactive confirmations
    #[arg(short = 'y', long, help = "Skip all interactive confirmations")]
    yes: bool,
}

impl CommandHandler for UninstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = SdkManager::new()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        manager.uninstall_sdk(&sdk, &self.sdk_version, self.yes)?;
        Ok(())
    }
}
