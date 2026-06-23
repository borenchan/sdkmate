// ──────────────────────────────────────────────────────
// sdkmate 配置系统——结构体、序列化、磁盘读写、配置操作 API
// ──────────────────────────────────────────────────────

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use util::config_helper::{
    PLACEHOLDER_SDK_DIR, PLACEHOLDER_SDKM_HOME_DIR, PLACEHOLDER_SDKS_INSTALL_DIR, TemplateRenderer,
};
use util::consts::{CONFIG_FILE_NAME, ENV_JAVA_HOME, SDKM_SYMLINK_DIR};
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::BUILTIN_SDK_CONFIG;

// ── 子模块声明 + 重新导出（pub use 同时满足内部使用和外部访问） ──
pub mod keys;
pub mod validation;
pub mod snapshot;

pub use keys::{ConfigKey, SdkField, parse_config_key, known_keys};
pub use validation::{ValueType, ValidatedValue, KeyMeta, field_type, key_meta, validate_by_type, mask_by_type, mask_token};
pub use snapshot::{ConfigSnapshot, take_config_snapshot, rollback_config, rollback_config_from_raw};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)] //ignore unknown fields
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
#[derive(Debug, Deserialize, Serialize, Clone)]
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

    /// Cache TTL in seconds for version API responses, default 3600 (1 hour)
    /// Smaller values = fresher data but more network calls;
    /// Larger values = faster response but potentially stale data.
    #[serde(default = "default_cache_ttl_secs")]
    pub cache_ttl_secs: u32,

    /// GitHub personal access token (optional).
    /// Increases GitHub API rate limit from 60/hr to 5000/hr.
    /// Create at: https://github.com/settings/tokens (no special permissions needed)
    #[serde(default)]
    pub github_token: Option<String>,
}

fn default_cache_ttl_secs() -> u32 {
    3600
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            proxy: None,
            ssl_verify: true,
            connect_timeout: 30,
            cache_ttl_secs: default_cache_ttl_secs(),
            github_token: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    //sdk unique name
    pub name: String,
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
    //二进制目录名，空值表示二进制在 SDK 根目录（如 Node.js）
    #[serde(default)]
    pub bin_dir: String,
    //extra env vars
    pub extra_vars: HashMap<String, String>,
    //extra paths relative to sdk symlink dir
    #[serde(default)]
    pub extra_paths: Vec<String>,
}
impl SdkConfig {
    /// 构造 SdkConfig，bin_dir 传空字符串表示二进制在 SDK 根目录
    pub fn new(name: String, version_url: String, download_url: String, bin_dir: String) -> SdkConfig {
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
        renderer = renderer
            .vars(dynamic_val)
            .var(PLACEHOLDER_SDKM_HOME_DIR, get_sdkm_home()?.to_string_lossy())
            .var(PLACEHOLDER_SDKS_INSTALL_DIR, get_installed_sdks_dir()?.to_string_lossy());
        let mut actual_extra_vars = HashMap::with_capacity(self.extra_vars.len());
        for (k, v) in &self.extra_vars {
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
        BUILTIN_SDK_CONFIG
            .iter()
            .map(|s| {
                let mut config = SdkConfig::new(
                    s.sdk.to_string(),
                    s.version_url.to_string(),
                    s.download_url.to_string(),
                    s.sdk.get_sdk_bin_dir().to_string(),
                );
                config.version_fallback_url = s.version_fallback_url.map(|u| u.to_string());
                config.download_fallback_url = s.download_fallback_url.map(|u| u.to_string());
                match s.sdk {
                    BuiltinSdk::Java => {
                        config
                            .extra_vars
                            .insert(ENV_JAVA_HOME.to_string(), PLACEHOLDER_SDK_DIR.to_string());
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
        if let Ok(config_file) = fs::read_to_string(get_sdkm_config_path()?) {
            let config = toml::from_str(config_file.as_str())
                .context("Failed to parse toml file,please check config.toml syntax!")?;
            return Ok(config);
        }
        anyhow::bail!("Failed to read sdkm config! please try again after executing `sdkm init` in sdkm home dir")
    }

    /// 将配置写入磁盘（兼容旧调用，内部使用原子写入）
    pub fn write_to_disk(&self) -> Result<()> {
        self.atomic_write_to_disk()
    }

    /// 原子写入配置到磁盘（写入-重命名模式）
    ///
    /// 1. 序列化为 TOML
    /// 2. 写入同目录下的临时文件 config.toml.tmp.{timestamp}
    /// 3. fs::rename 临时文件 → 正式文件（POSIX 原子，Windows 同卷安全）
    /// 4. 失败时清理临时文件
    pub fn atomic_write_to_disk(&self) -> Result<()> {
        let config_path = get_sdkm_config_path()?;
        let dir = config_path
            .parent()
            .context("config file has no parent directory")?;

        // 序列化为 TOML
        let content =
            toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        // 生成临时文件名（基于时间戳，防并发冲突）
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_name = format!("{}.tmp.{:016x}", CONFIG_FILE_NAME, ts & 0xFFFFFFFFFFFFFFFF);
        let tmp_path = dir.join(&tmp_name);

        // 写入临时文件
        fs::write(&tmp_path, &content).context("Failed to write temporary config file")?;

        // rename 临时文件 → 正式文件
        let rename_result = fs::rename(&tmp_path, &config_path);
        if rename_result.is_err() {
            // rename 失败，清理临时文件后返回错误
            let _ = fs::remove_file(&tmp_path); // 尽力清理
            return rename_result.context("Failed to rename temporary config to final path");
        }

        Ok(())
    }

    pub fn find_sdk(&self, sdk: &Sdk) -> Option<&SdkConfig> {
        self.sdks.iter().find(|s| s.name == sdk.to_string())
    }
    pub fn find_sdk_mut(&mut self, sdk: &Sdk) -> Option<&mut SdkConfig> {
        self.sdks.iter_mut().find(|s| s.name == sdk.to_string())
    }
    pub fn find_sdk_ok(&self, sdk: &Sdk) -> Result<&SdkConfig> {
        self.find_sdk(sdk)
            .ok_or_else(|| anyhow::anyhow!("Unregistered SDK:`{}` please check config!", sdk))
    }
    pub fn find_sdk_mut_ok(&mut self, sdk: &Sdk) -> Result<&mut SdkConfig> {
        self.find_sdk_mut(sdk)
            .ok_or_else(|| anyhow::anyhow!("Unregistered SDK:`{}` please check config!", sdk))
    }
    pub fn exist_sdk(&self, sdk: &Sdk) -> bool {
        self.find_sdk(sdk).is_some()
    }

    /// 判断 SDK 名称是否为内置 SDK
    pub fn is_builtin_sdk(name: &str) -> bool {
        matches!(name, "java" | "maven" | "node" | "python")
    }

    // ── 配置操作 API ──

    /// 获取配置键的当前值（返回脱敏后的字符串表示）
    pub fn get_value(&self, key: &ConfigKey) -> Result<String> {
        let ty = field_type(key);
        let raw = self.get_raw_value(key)?;
        Ok(mask_by_type(&raw, &ty))
    }

    /// 获取配置键的原始值（不脱敏）
    fn get_raw_value(&self, key: &ConfigKey) -> Result<String> {
        match key {
            ConfigKey::SymlinkDir => Ok(self.symlink_dir.clone()),
            ConfigKey::NetworkProxy => Ok(self.network.proxy.clone().unwrap_or_default()),
            ConfigKey::NetworkSslVerify => Ok(self.network.ssl_verify.to_string()),
            ConfigKey::NetworkConnectTimeout => Ok(self.network.connect_timeout.to_string()),
            ConfigKey::NetworkCacheTtlSecs => Ok(self.network.cache_ttl_secs.to_string()),
            ConfigKey::NetworkGithubToken => Ok(self.network.github_token.clone().unwrap_or_default()),
            ConfigKey::Sdk { name, field } => {
                let sdk = self.find_sdk_by_name(name)?;
                match field {
                    SdkField::VersionUrl => Ok(sdk.version_url.clone().unwrap_or_default()),
                    SdkField::VersionFallbackUrl => Ok(sdk.version_fallback_url.clone().unwrap_or_default()),
                    SdkField::DownloadUrl => Ok(sdk.download_url.clone()),
                    SdkField::DownloadFallbackUrl => Ok(sdk.download_fallback_url.clone().unwrap_or_default()),
                    SdkField::CurrentVersion => Ok(sdk.current_version.clone().unwrap_or_default()),
                    SdkField::BinDir => Ok(sdk.bin_dir.clone()),
                }
            }
            ConfigKey::SdkExtraVar { name, var_key } => {
                let sdk = self.find_sdk_by_name(name)?;
                Ok(sdk.extra_vars.get(var_key).cloned().unwrap_or_default())
            }
            ConfigKey::SdkExtraPath { name, index } => {
                let sdk = self.find_sdk_by_name(name)?;
                Ok(sdk.extra_paths.get(*index).cloned().unwrap_or_default())
            }
        }
    }

    /// 设置配置键的值（修改内存 + 原子写入磁盘 + 回滚保障）
    pub fn set_value(&mut self, key: &ConfigKey, validated: ValidatedValue) -> Result<()> {
        let snapshot = take_config_snapshot()?;

        // 修改内存中的配置
        self.apply_set_value(key, validated);

        // 原子写入磁盘，失败时自动回滚
        if let Err(e) = self.atomic_write_to_disk() {
            util::warning!("Failed to write config, rolling back...");
            if rollback_config(&snapshot).is_err() {
                if rollback_config_from_raw(&snapshot).is_err() {
                    util::warning!("Config rollback failed! Your config file may be corrupted.");
                    util::detail!("Manual recovery may be needed. Re-run `sdkm init --force` to reset.");
                }
            }
            return Err(util::consts::BugReportError::wrap(e));
        }

        Ok(())
    }

    /// 将已校验的值应用到内存中的配置
    fn apply_set_value(&mut self, key: &ConfigKey, validated: ValidatedValue) {
        let value = validated.into_string();
        match key {
            ConfigKey::SymlinkDir => self.symlink_dir = value,
            ConfigKey::NetworkProxy => self.network.proxy = Some(value),
            ConfigKey::NetworkSslVerify => self.network.ssl_verify = value == "true",
            ConfigKey::NetworkConnectTimeout => self.network.connect_timeout = value.parse().unwrap_or(30),
            ConfigKey::NetworkCacheTtlSecs => self.network.cache_ttl_secs = value.parse().unwrap_or(3600),
            ConfigKey::NetworkGithubToken => self.network.github_token = Some(value),
            ConfigKey::Sdk { name, field } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    match field {
                        SdkField::VersionUrl => sdk.version_url = Some(value),
                        SdkField::VersionFallbackUrl => sdk.version_fallback_url = Some(value),
                        SdkField::DownloadUrl => sdk.download_url = value,
                        SdkField::DownloadFallbackUrl => sdk.download_fallback_url = Some(value),
                        SdkField::CurrentVersion => sdk.current_version = Some(value),
                        SdkField::BinDir => sdk.bin_dir = value,
                    }
                }
            }
            ConfigKey::SdkExtraVar { name, var_key } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    sdk.extra_vars.insert(var_key.clone(), value);
                }
            }
            ConfigKey::SdkExtraPath { name, index } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    if let Some(path_entry) = sdk.extra_paths.get_mut(*index) {
                        *path_entry = value;
                    }
                }
            }
        }
    }

    /// 删除配置键的值（恢复为默认值/None + 原子写入磁盘 + 回滚保障）
    /// 内置 SDK 字段和不可删除字段返回错误
    pub fn delete_value(&mut self, key: &ConfigKey) -> Result<()> {
        // 检查是否可删除
        let meta = self.key_meta_for(key);
        if !meta.deletable {
            bail!(
                "Cannot delete config key '{}'. It is a required field or belongs to a built-in SDK.\nUse `sdkm config set {} <value>` to modify it instead.",
                key.display(),
                key.display()
            );
        }

        let snapshot = take_config_snapshot()?;

        // 恢复为默认值
        self.apply_delete_value(key, &meta);

        // 原子写入磁盘
        if let Err(e) = self.atomic_write_to_disk() {
            util::warning!("Failed to write config, rolling back...");
            if rollback_config(&snapshot).is_err() {
                if rollback_config_from_raw(&snapshot).is_err() {
                    util::warning!("Config rollback failed! Your config file may be corrupted.");
                }
            }
            return Err(util::consts::BugReportError::wrap(e));
        }

        Ok(())
    }

    /// 将 delete 操作应用到内存中的配置（恢复为默认值/None）
    fn apply_delete_value(&mut self, key: &ConfigKey, _meta: &KeyMeta) {
        match key {
            ConfigKey::NetworkProxy => self.network.proxy = None,
            ConfigKey::NetworkGithubToken => self.network.github_token = None,
            ConfigKey::Sdk { name, field } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    match field {
                        SdkField::VersionUrl => sdk.version_url = None,
                        SdkField::VersionFallbackUrl => sdk.version_fallback_url = None,
                        SdkField::DownloadFallbackUrl => sdk.download_fallback_url = None,
                        SdkField::CurrentVersion => sdk.current_version = None,
                        SdkField::DownloadUrl | SdkField::BinDir => {
                            // 不可删除字段，不应到达此处（已在 delete_value 中校验）
                        }
                    }
                }
            }
            ConfigKey::SdkExtraVar { name, var_key } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    sdk.extra_vars.remove(var_key);
                }
            }
            ConfigKey::SdkExtraPath { name, index } => {
                if let Some(sdk) = self.find_sdk_mut_by_name(name) {
                    if *index < sdk.extra_paths.len() {
                        sdk.extra_paths.remove(*index);
                    }
                }
            }
            // 不可删除的顶层键不应到达此处
            ConfigKey::SymlinkDir
            | ConfigKey::NetworkSslVerify
            | ConfigKey::NetworkConnectTimeout
            | ConfigKey::NetworkCacheTtlSecs => {}
        }
    }

    /// 获取键名元数据（内置 SDK 字段不可删除）
    pub fn key_meta_for(&self, key: &ConfigKey) -> KeyMeta {
        let is_builtin = match key {
            ConfigKey::Sdk { name, .. } => SdkmConfig::is_builtin_sdk(name),
            ConfigKey::SdkExtraVar { name, .. } => SdkmConfig::is_builtin_sdk(name),
            ConfigKey::SdkExtraPath { name, .. } => SdkmConfig::is_builtin_sdk(name),
            _ => false,
        };
        key_meta(key, is_builtin)
    }

    /// 列出所有配置键及其当前值（脱敏后的）
    pub fn list_all_values(&self) -> Vec<(String, String)> {
        let mut entries = Vec::new();

        // 顶层键
        entries.push(("symlink_dir".to_string(), self.symlink_dir.clone()));

        // network 子表
        entries.push(("network.proxy".to_string(), self.network.proxy.clone().unwrap_or("(none)".to_string())));
        entries.push(("network.ssl_verify".to_string(), self.network.ssl_verify.to_string()));
        entries.push(("network.connect_timeout".to_string(), self.network.connect_timeout.to_string()));
        entries.push(("network.cache_ttl_secs".to_string(), self.network.cache_ttl_secs.to_string()));
        entries.push(("network.github_token".to_string(), mask_token(&self.network.github_token.clone().unwrap_or_default())));

        // SDK 子表（按 name 字母序排列）
        let mut sorted_sdks: Vec<&SdkConfig> = self.sdks.iter().collect();
        sorted_sdks.sort_by(|a, b| a.name.cmp(&b.name));

        for sdk in &sorted_sdks {
            let prefix = format!("sdk.{}", sdk.name);

            entries.push((format!("{}.version_url", prefix), sdk.version_url.clone().unwrap_or("(none)".to_string())));
            entries.push((format!("{}.version_fallback_url", prefix), sdk.version_fallback_url.clone().unwrap_or("(none)".to_string())));
            entries.push((format!("{}.download_url", prefix), sdk.download_url.clone()));
            entries.push((format!("{}.download_fallback_url", prefix), sdk.download_fallback_url.clone().unwrap_or("(none)".to_string())));
            entries.push((format!("{}.current_version", prefix), sdk.current_version.clone().unwrap_or("(none)".to_string())));
            entries.push((format!("{}.bin_dir", prefix), sdk.bin_dir.clone()));

            // extra_vars
            for (var_key, var_val) in &sdk.extra_vars {
                entries.push((format!("{}.extra_vars.{}", prefix, var_key), var_val.clone()));
            }
            // extra_paths
            for (i, path) in sdk.extra_paths.iter().enumerate() {
                entries.push((format!("{}.extra_paths.{}", prefix, i), path.clone()));
            }
        }

        entries
    }

    /// 新增自定义 SDK 条目（原子写入 + 回滚保障）
    /// 校验：name 不与已有 SDK 重名
    pub fn add_sdk(&mut self, sdk_config: SdkConfig) -> Result<()> {
        // 检查是否重名
        if self.sdks.iter().any(|s| s.name == sdk_config.name) {
            bail!("SDK '{}' already exists in config. Use `sdkm config set sdk.{}.xxx <value>` to modify it.", sdk_config.name, sdk_config.name);
        }

        let snapshot = take_config_snapshot()?;
        self.sdks.push(sdk_config);

        if let Err(e) = self.atomic_write_to_disk() {
            util::warning!("Failed to write config, rolling back...");
            if rollback_config(&snapshot).is_err() {
                if rollback_config_from_raw(&snapshot).is_err() {
                    util::warning!("Config rollback failed!");
                }
            }
            return Err(util::consts::BugReportError::wrap(e));
        }

        Ok(())
    }

    /// 移除 SDK 条目（内置 SDK 不可移除）
    pub fn remove_sdk(&mut self, name: &str) -> Result<()> {
        // 内置 SDK 不可移除
        if SdkmConfig::is_builtin_sdk(name) {
            bail!("Cannot remove built-in SDK '{}'. Use `sdkm config set` to modify its fields.", name);
        }

        // 检查 SDK 是否存在
        let index = self.sdks.iter().position(|s| s.name == name);
        if index.is_none() {
            bail!("SDK '{}' not found in config.", name);
        }

        let snapshot = take_config_snapshot()?;
        self.sdks.remove(index.unwrap());

        if let Err(e) = self.atomic_write_to_disk() {
            util::warning!("Failed to write config, rolling back...");
            if rollback_config(&snapshot).is_err() {
                if rollback_config_from_raw(&snapshot).is_err() {
                    util::warning!("Config rollback failed!");
                }
            }
            return Err(util::consts::BugReportError::wrap(e));
        }

        Ok(())
    }

    // ── 内部辅助方法 ──

    /// 按 SDK name 字符串查找
    pub fn find_sdk_by_name(&self, name: &str) -> Result<&SdkConfig> {
        self.sdks
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("SDK '{}' not found in config.", name))
    }

    /// 按 SDK name 字符串可变查找
    fn find_sdk_mut_by_name(&mut self, name: &str) -> Option<&mut SdkConfig> {
        self.sdks.iter_mut().find(|s| s.name == name)
    }
}
