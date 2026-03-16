use clap::error::ContextValue::String;
use clap::Parser;
use sdkcore::manager::init::init_sdkm;
use sdkcore::manager::list::{query_local_sdk_list, query_local_sdk_version_list};
use util::sdk::Sdk;
use crate::CommandHandler;

#[derive(Debug,Parser)]
pub struct ListHandler {
    #[arg(long, value_enum,help = "sdk name for list operation")]
    sdk: Option<Sdk>,
}

impl CommandHandler for ListHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut sdks = vec![];
        if let Some(sdk) = self.sdk {
            sdks = query_local_sdk_version_list(sdk)?;
        }else {
            sdks = query_local_sdk_list()?;
        }
        Ok(())
    }
}

