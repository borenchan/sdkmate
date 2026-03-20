use crate::env::{EnvOperation, OsEnvOperation};
use crate::manager::config::SdkmConfig;

pub mod config;
pub mod init;
pub mod install;
pub mod list;
pub mod switch;


// #[derive(Debug)]
pub struct SdkManager {
    pub config: SdkmConfig,
    pub env_operation: Box<dyn EnvOperation>,
}

impl SdkManager {
    pub fn new() -> anyhow::Result<SdkManager> {
        let config = SdkmConfig::read_from_disk()?;
        Ok(SdkManager {
            config,
            env_operation: Box::new(OsEnvOperation{}),
        })
    }
}