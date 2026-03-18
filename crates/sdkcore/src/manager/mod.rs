use crate::manager::config::SdkmConfig;

pub mod config;
pub mod init;
pub mod install;
pub mod list;
pub mod switch;


#[derive(Debug)]
pub struct SdkManager {
    pub config: SdkmConfig
}

impl SdkManager {
    pub fn new() -> anyhow::Result<SdkManager> {
        let config = SdkmConfig::read_from_disk()?;
        Ok(SdkManager {
            config
        })
    }
}