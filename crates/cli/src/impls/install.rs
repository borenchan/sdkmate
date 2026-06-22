use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct InstallHandler {
    /// 要安装的 SDK 名称。内置: java, node, python, maven; 自定义 SDK 见 config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven. Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// 目标版本号，支持模糊匹配，如 '21' 自动解析为最新 21.x
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// 安装后不自动切换到新版本
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
