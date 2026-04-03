use crate::CommandHandler;
use anyhow::{Result, anyhow};
use clap::{Parser, ValueEnum};
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;

#[derive(Debug, Parser)]
pub struct ListHandler {
    /// The following available SDKs are supported:  java| node | python | rust | maven
    /// Custom SDKs defined in config are also accepted.
    #[arg(value_name = "SDK", help = "query the list of available versions of a specific SDK")]
    sdk: Option<String>,

    #[arg(
        long,value_enum,
        default_value_t = SdkSource::Local,
        help = "select sdk-list from local disk or remote"
    )]
    source: SdkSource,
}
#[derive(Debug, Clone, ValueEnum)]
enum SdkSource {
    /// query from local disk
    Local,
    /// query from remote server
    Remote,
}

impl CommandHandler for ListHandler {
    fn run(&self) -> Result<()> {
        let manager = SdkManager::new()?;
        if let Some(sdk_name) = &self.sdk {
            let sdk = manager.match_valid_sdk(sdk_name)?;
            manager.show_local_sdk_version_list(&sdk)?;
        } else {
            manager.show_local_sdk_list()?;
        }
        Ok(())
    }
}
