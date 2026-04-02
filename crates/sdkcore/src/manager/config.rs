use std::collections::HashMap;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use util::consts::{CONFIG_FILE_NAME, ENV_JAVA_HOME, SDKM_SYMLINK_DIR};
use util::path::get_sdkm_config_path;
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::BUILTIN_SDK_CONFIG;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields,default)]  //ignore unknown fields
pub struct SdkmConfig {
    //sdkm self home dir readonly
    #[serde(default)]
    pub home_dir: Option<String>,
    //sdkm symlink dir
    #[serde(default)]
    pub symlink_dir: String,
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
    //sdk unique name
    pub name : String,
    //sdk versions release url
    pub version_url: Option<String>,
    //sdk real download url
    pub download_url: String,
    //current active version
    pub current_version: Option<String>,
    //binary dir
    pub bin_dir: String,
    //extra env vars
    pub extra_vars: HashMap<String, String>,
}
impl SdkConfig {
    pub fn new(name: String, version_url: String,download_url: String, bin_dir: String) -> SdkConfig {
        SdkConfig {
            name,
            version_url: Some(version_url),
            download_url,
            bin_dir,
            current_version: None,
            extra_vars: HashMap::with_capacity(0),
        }
    }
}
impl Default for SdkmConfig {
    fn default() -> SdkmConfig {
        SdkmConfig {
            home_dir: None,
            symlink_dir: SDKM_SYMLINK_DIR.to_string(),
            network: NetworkConfig::default(),
            sdks: Self::get_default_builtin_sdks(),
        }

    }
}


impl SdkmConfig {
    pub fn get_default_builtin_sdks() -> Vec<SdkConfig> {
        BUILTIN_SDK_CONFIG.iter()
            .map(|s| {
                let mut config = SdkConfig::new(s.sdk.to_string(), s.version_list_url.to_string(), s.download_url.to_string(), s.sdk.get_sdk_bin_dir().to_string());
                match s.sdk {
                    BuiltinSdk::Java => {
                        config.extra_vars.insert(ENV_JAVA_HOME.to_string(), "{sdk_dir}".to_string());
                    }
                    _ => {}
                }
                config
            })
            .collect()
    }

    pub fn read_from_disk() -> Result<SdkmConfig> {
        if let Ok(config_file)  = fs::read_to_string(get_sdkm_config_path()?) {
            let config = toml::from_str(config_file.as_str()).context("Failed to parse toml file,please check config.toml syntax!")?;
            return Ok(config)
        }
        anyhow::bail!("Failed to read sdkm config! please try again after executing `sdkm init` ")
    }

    pub fn write_to_disk(&self) -> Result<()> {
        let config_file = toml::to_string_pretty(self).context("Failed to serialize toml file")?;
        fs::write(CONFIG_FILE_NAME, config_file).context("Failed to write toml file")?;
        Ok(())
    }

    pub fn find_sdk(&self, sdk: Sdk) -> Option<&SdkConfig> {
        self.sdks.iter().find(|s| s.name == sdk.to_string())
    }
    pub fn find_sdk_mut(&mut self, sdk: Sdk) -> Option<&mut SdkConfig> {
        self.sdks.iter_mut().find(|s| s.name == sdk.to_string())
    }
}

