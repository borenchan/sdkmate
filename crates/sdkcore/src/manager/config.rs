use std::collections::HashMap;
use std::env::var;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use util::config_helper::{TemplateRenderer, PLACEHOLDER_SDKM_HOME_DIR, PLACEHOLDER_SDKS_INSTALL_DIR, PLACEHOLDER_SDK_DIR};
use util::consts::{CONFIG_FILE_NAME, ENV_JAVA_HOME, SDKM_SYMLINK_DIR};
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};
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
/// [network] network settings
#[derive(Debug, Deserialize, Serialize,Clone)]
pub struct NetworkConfig {
    /// Proxy URL, e.g. "http://127.0.0.1:7890"
    #[serde(default)]
    pub proxy: Option<String>,

    /// Verify SSL, default true
    #[serde(default)]
    pub ssl_verify: bool,

    /// Connect timeout in seconds, default 30
    #[serde(default)]
    pub connect_timeout: u32,

    /// GitHub personal access token (optional).
    /// Increases GitHub API rate limit from 60/hr to 5000/hr.
    /// Create at: https://github.com/settings/tokens (no special permissions needed)
    #[serde(default)]
    pub github_token: Option<String>,
}
impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            proxy: None,
            ssl_verify: true,
            connect_timeout: 30,
            github_token: None,
        }
    }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SdkConfig {
    //sdk unique name
    pub name : String,
    //版本发现主源 URL
    #[serde(default)]
    pub version_url: Option<String>,
    //版本发现备源 URL（主源失败时回退）
    #[serde(default)]
    pub version_fallback_url: Option<String>,
    //下载主源 URL 模板
    pub download_url: String,
    //下载备源 URL 模板（下载主源失败时回退）
    #[serde(default)]
    pub download_fallback_url: Option<String>,
    //current active version
    #[serde(default)]
    pub current_version: Option<String>,
    //binary dir
    pub bin_dir: String,
    //extra env vars
    pub extra_vars: HashMap<String, String>,
    //extra paths relative to sdk symlink dir
    #[serde(default)]
    pub extra_paths: Vec<String>,
}
impl SdkConfig {
    pub fn new(name: String, version_url: String,download_url: String, bin_dir: String) -> SdkConfig {
        SdkConfig {
            name,
            version_url: Some(version_url),
            version_fallback_url: None,
            download_url,
            download_fallback_url: None,
            bin_dir,
            current_version: None,
            extra_vars: HashMap::with_capacity(0),
            extra_paths: Vec::new(),
        }
    }

    pub fn get_actual_extra_vars(&self, dynamic_val: &HashMap<&str, &str>) -> Result<HashMap<String, String>> {
        let mut renderer = TemplateRenderer::new();
        renderer = renderer.vars(dynamic_val)
            .var(PLACEHOLDER_SDKM_HOME_DIR,get_sdkm_home()?.to_string_lossy())
            .var(PLACEHOLDER_SDKS_INSTALL_DIR,get_installed_sdks_dir()?.to_string_lossy());
        let mut actual_extra_vars = HashMap::with_capacity(self.extra_vars.len());
        for (k,v) in &self.extra_vars {
            let val = renderer.render(v)?;
            actual_extra_vars.insert(k.to_string(), val);
        }
        Ok(actual_extra_vars)
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
                let mut config = SdkConfig::new(s.sdk.to_string(), s.version_url.to_string(), s.download_url.to_string(), s.sdk.get_sdk_bin_dir().to_string());
                config.version_fallback_url = s.version_fallback_url.map(|u| u.to_string());
                config.download_fallback_url = s.download_fallback_url.map(|u| u.to_string());
                match s.sdk {
                    BuiltinSdk::Java => {
                        config.extra_vars.insert(ENV_JAVA_HOME.to_string(), PLACEHOLDER_SDK_DIR.to_string());
                    }
                    // Python install_only 版本：二次提升后，pip.exe 在 Scripts 子目录（仅 Windows）
                    BuiltinSdk::Python => {
                        if cfg!(target_os = "windows") {
                            config.extra_paths.push("Scripts".to_string());
                        }
                        // Unix 的 pip 在 bin/ 下，已由 bin_dir 覆盖
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
        anyhow::bail!("Failed to read sdkm config! please try again after executing `sdkm init` in sdkm home dir")
    }

    pub fn write_to_disk(&self) -> Result<()> {
        let config_file = toml::to_string_pretty(self).context("Failed to serialize toml file")?;
        fs::write(get_sdkm_config_path()?, config_file).context("Failed to write toml file")?;
        Ok(())
    }

    pub fn find_sdk(&self, sdk: &Sdk) -> Option<&SdkConfig> {
        self.sdks.iter().find(|s| s.name == sdk.to_string())
    }
    pub fn find_sdk_mut(&mut self, sdk: &Sdk) -> Option<&mut SdkConfig> {
        self.sdks.iter_mut().find(|s| s.name == sdk.to_string())
    }
    pub fn find_sdk_ok(&self, sdk: &Sdk) -> Result<&SdkConfig> {
        self.find_sdk(sdk).ok_or_else(|| anyhow::anyhow!("Unregistered SDK:`{}` please check config!", sdk))
    }
    pub fn find_sdk_mut_ok(&mut self, sdk: &Sdk) -> Result<&mut SdkConfig> {
        self.find_sdk_mut(sdk).ok_or_else(|| anyhow::anyhow!("Unregistered SDK:`{}` please check config!", sdk))
    }
    pub fn exist_sdk(&self, sdk: &Sdk) -> bool {
        self.find_sdk(sdk).is_some()
    }
}

