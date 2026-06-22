use crate::CommandHandler;
use anyhow::Result;
use clap::ArgAction::SetTrue;
use clap::Parser;
use sdkcore::manager::SdkManager;

#[derive(Debug, Parser)]
pub struct InitHandler {
    /// 强制重新初始化，覆盖已有配置文件，跳过目录名检测
    #[arg(
        long,
        short,
        default_value_t = false,
        action = SetTrue,
        help = "Force reinitialize sdkm (overwrites config, skips dir check)"
    )]
    force: bool,
}

impl CommandHandler for InitHandler {
    fn run(&self) -> Result<()> {
        SdkManager::init_sdkm(self.force)?;
        Ok(())
    }
}
