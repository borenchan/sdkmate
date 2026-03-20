use std::fs;
use serde::{Deserialize, Serialize};
use toml::toml;
use anyhow::{Context, Result};
use util::consts::SDKM_SYMLINK_DIR;

pub const CONFIG_FILE_NAME: &'static str = "config.toml";
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SdkmConfig {
    pub sdkm_symlink_dir: Option<String>,
    pub java: Option<Java>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Java {
    pub current_active_version: Option<String>,
    pub sdk_download_url: String,
}

impl SdkmConfig {

    pub fn default() -> SdkmConfig {
        SdkmConfig {
            sdkm_symlink_dir: Some(SDKM_SYMLINK_DIR.to_string()),
            java: None,
        }
    }

    pub fn read_from_disk() -> Result<SdkmConfig> {
        if let Ok(config_file)  = fs::read_to_string(CONFIG_FILE_NAME) {
            let config = toml::from_str(config_file.as_str()).context("Failed to parse toml file,please check config.toml syntax!")?;
            Ok(config)
        }else {
            Ok(SdkmConfig::default())
        }

    }

    pub fn write_to_disk(&self) -> Result<()> {
        let config_file = toml::to_string_pretty(self).context("Failed to serialize toml file")?;
        fs::write(CONFIG_FILE_NAME, config_file).context("Failed to write toml file")?;
        Ok(())
    }
}

