use crate::CommandHandler;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct CurrentHandler {
    /// 要查询的 SDK 名称。内置: java, node, python, maven; 留空则显示全部活跃版本
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven. Omit to show all active versions"
    )]
    sdk: Option<String>,
}

impl CommandHandler for CurrentHandler {
    fn run(&self) -> anyhow::Result<()> {
        let manager = SdkManager::new()?;
        if let Some(sdk_name) = &self.sdk {
            let sdk = manager.match_valid_sdk(sdk_name)?;
            manager.show_local_sdks_current(Some(sdk))?;
        } else {
            manager.show_local_sdks_current(None)?;
        }
        Ok(())
    }
}
