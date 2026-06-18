use clap::Parser;
use sdkcore::manager::SdkManager;
use crate::CommandHandler;

#[derive(Debug, Parser)]
pub struct SwitchHandler {
    /// 要切换的 SDK 名称。内置: java, node, python, maven; 自定义 SDK 见 config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// 目标版本号，必须是已安装的版本
    #[arg(value_name = "VERSION", help = "Target version (must be installed locally)")]
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
