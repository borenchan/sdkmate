use clap::{Parser, ValueEnum};
use anyhow::Result;
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct ListHandler {
    #[arg(value_enum, help = "query the list of available versions of a specific SDK")]
    sdk: Option<Sdk>,

    #[arg(
        long,value_enum,
        default_value_t = SdkSource::Local,
        help = "select the sdk-list of available supplier source"
    )]
    source:  SdkSource,
}
#[derive(Debug,Clone,ValueEnum)]
enum SdkSource {
    /// query from local disk
    Local,
    /// query from remote server
    Remote,
}

impl CommandHandler for ListHandler {
    fn run(&self) -> Result<()> {
        let manager = SdkManager::new()?;
        if let Some(sdk) = self.sdk {
            manager.show_local_sdk_version_list(sdk)?;
        }else {
            manager.show_local_sdk_list()?;
        }
        Ok(())
    }
}

