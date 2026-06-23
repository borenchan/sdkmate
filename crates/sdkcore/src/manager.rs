use crate::config::SdkmConfig;
use crate::env::{EnvOperation, OsEnvOperation};
use std::str::FromStr;
use util::sdk::Sdk;

pub struct SdkManager {
    pub config: SdkmConfig,
    pub env_operation: Box<dyn EnvOperation>,
}

impl SdkManager {
    pub fn new() -> anyhow::Result<SdkManager> {
        let config = SdkmConfig::read_from_disk()?;
        Ok(SdkManager {
            config,
            env_operation: Box::new(OsEnvOperation {}),
        })
    }
    /// 校验 SDK 名称是否在配置中注册
    pub fn match_valid_sdk(&self, sdk_name: &str) -> anyhow::Result<Sdk> {
        let sdk = Sdk::from_str(sdk_name)?;
        if !self.config.exist_sdk(&sdk) {
            anyhow::bail!(
                "Unregistered SDK:`{}` , please manually register it to the SDKM configuration!",
                sdk
            )
        }
        Ok(sdk)
    }
}
