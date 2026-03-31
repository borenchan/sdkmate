use std::{env, fs};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use toml::toml;
use anyhow::{Context, Result};
use util::consts::{SDKM_STORE_DIR, SDKM_SYMLINK_DIR};

pub const CONFIG_FILE_NAME: &'static str = "config.toml";
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields,default)]  //ignore unknown fields
pub struct SdkmConfig {
    //sdkm self install dir
    #[serde(default)]
    pub install_dir: Option<String>,
    //sdkm symlink dir
    #[serde(default)]
    pub symlink_dir: Option<String>,
    //network
    #[serde(default)]
    pub network: NetworkConfig,
    //multi sdk config
    #[serde(default, rename = "sdk")]
    pub sdks: Vec<SdkConfig>,
}
/// [network] 网络相关设置
#[derive(Debug, Deserialize, Serialize,Clone)]
pub struct NetworkConfig {
    /// 代理地址，如 "http://127.0.0.1:7890"
    #[serde(default)]
    pub proxy: Option<String>,

    /// 是否验证 SSL，默认 true
    #[serde(default)]
    pub ssl_verify: bool,

    /// 连接超时秒数，默认 30
    #[serde(default)]
    pub connect_timeout: u32,
}
impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            proxy: None,
            ssl_verify: true,
            connect_timeout: 30,
        }
    }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SdkConfig {
    pub name : String,
    pub sdk_download_url: String,
    pub current_version: Option<String>,
}

impl Default for SdkmConfig {
    fn default() -> SdkmConfig {
        SdkmConfig {
            install_dir: None,
            symlink_dir: Some(SDKM_SYMLINK_DIR.to_string()),
            network: NetworkConfig::default(),
            sdks: vec![],
        }
    }
}

impl SdkmConfig {

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

    /// Get the dir for the SDK Manager's SDKs installed.
    pub fn get_installed_sdks_dir(&self) -> Result<PathBuf> {
        let install_opt = &self.install_dir;
        match install_opt {
            None => Err(anyhow::anyhow!("Not found config key `install_dir` in sdkm's config.toml")),
            Some(dir) => Ok(PathBuf::from( dir).join(SDKM_STORE_DIR))
        }
    }
}

