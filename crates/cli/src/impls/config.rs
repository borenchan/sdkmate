use crate::CommandHandler;
use anyhow::{Context, Result};
use clap::Parser;
use sdkcore::config::{
    parse_config_key, validate_by_type, field_type, mask_by_type,
    ConfigKey, SdkConfig, SdkmConfig, ValidatedValue, ValueType,
};
use sdkcore::manager::SdkManager;
use std::collections::HashMap;
use std::process::Command;
use util::{detail, info, success, warning};

#[derive(Debug, Parser)]
pub struct ConfigHandler {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Debug, Parser)]
enum ConfigCommands {
    /// Set a config value, e.g. sdkm config set network.proxy http://127.0.0.1:7890
    #[command(name = "set")]
    Set(ConfigSetHandler),

    /// Get a config value, e.g. sdkm config get network.proxy
    #[command(name = "get")]
    Get(ConfigGetHandler),

    /// List all config keys and their current values
    #[command(name = "list", visible_aliases = ["ls", "l"])]
    List,

    /// Delete a config value (reset to default), e.g. sdkm config delete network.proxy
    #[command(name = "delete", visible_alias = "del")]
    Delete(ConfigDeleteHandler),

    /// Open config file in system editor, validate TOML on save
    #[command(name = "edit", visible_alias = "e")]
    Edit,

    /// Add a custom SDK entry, e.g. sdkm config add-sdk mytool --download-url https://... --bin-dir bin
    #[command(name = "add-sdk")]
    AddSdk(AddSdkHandler),

    /// Remove a custom SDK entry (built-in SDKs cannot be removed)
    #[command(name = "remove-sdk")]
    RemoveSdk(RemoveSdkHandler),
}

#[derive(Debug, Parser)]
struct ConfigSetHandler {
    /// Config key in dot notation, e.g. network.proxy or sdk.java.download_url
    #[arg(value_name = "KEY", help = "Config key in dot notation, e.g. network.proxy")]
    key: String,

    /// Config value to set
    #[arg(value_name = "VALUE", help = "Config value to set")]
    value: String,
}

#[derive(Debug, Parser)]
struct ConfigGetHandler {
    /// Config key in dot notation
    #[arg(value_name = "KEY", help = "Config key in dot notation")]
    key: String,
}

#[derive(Debug, Parser)]
struct ConfigDeleteHandler {
    /// Config key to delete (resets to default)
    #[arg(value_name = "KEY", help = "Config key to delete (resets to default)")]
    key: String,
}

#[derive(Debug, Parser)]
struct AddSdkHandler {
    /// SDK name (must be unique, no duplicates)
    #[arg(value_name = "NAME", help = "SDK name (must be unique)")]
    name: String,

    /// Download URL template (required, supports {version} placeholders)
    #[arg(long, help = "Download URL template (required, supports {version} placeholders)")]
    download_url: String,

    /// SDK binary directory name (omit = binaries in SDK root dir, e.g. bin, Scripts)
    #[arg(long, help = "SDK binary directory name (omit for root-dir binaries, e.g. bin, Scripts)")]
    bin_dir: Option<String>,

    /// Version discovery URL (optional)
    #[arg(long, help = "Version discovery URL (optional)")]
    version_url: Option<String>,

    /// Version discovery fallback URL (optional)
    #[arg(long, help = "Version discovery fallback URL (optional)")]
    version_fallback_url: Option<String>,

    /// Download fallback URL template (optional)
    #[arg(long, help = "Download fallback URL template (optional)")]
    download_fallback_url: Option<String>,

    /// Extra env var in KEY=VALUE format, repeatable
    #[arg(long, value_name = "KEY=VALUE", help = "Extra env vars (KEY=VALUE, repeatable)")]
    extra_var: Vec<String>,

    /// Extra path relative to SDK symlink dir, repeatable
    #[arg(long, help = "Extra paths relative to symlink dir (repeatable)")]
    extra_path: Vec<String>,
}

#[derive(Debug, Parser)]
struct RemoveSdkHandler {
    /// SDK name to remove (built-in SDKs cannot be removed)
    #[arg(value_name = "NAME", help = "SDK name to remove (built-in SDKs cannot be removed)")]
    name: String,
}

impl CommandHandler for ConfigHandler {
    fn run(&self) -> Result<()> {
        match &self.command {
            ConfigCommands::Set(h) => h.run(),
            ConfigCommands::Get(h) => h.run(),
            ConfigCommands::List => run_list(),
            ConfigCommands::Delete(h) => h.run(),
            ConfigCommands::Edit => run_edit(),
            ConfigCommands::AddSdk(h) => h.run(),
            ConfigCommands::RemoveSdk(h) => h.run(),
        }
    }
}

impl ConfigSetHandler {
    fn run(&self) -> Result<()> {
        let mut manager = SdkManager::new()?;

        // 解析键名
        let key = parse_config_key(&self.key)?;

        // 校验 SDK 键名对应的 SDK 是否存在（sdk.xxx 键需要先注册）
        if let ConfigKey::Sdk { name, .. } | ConfigKey::SdkExtraVar { name, .. } | ConfigKey::SdkExtraPath { name, .. } = &key {
            if manager.config.find_sdk_by_name(name).is_err() {
                anyhow::bail!(
                    "SDK '{}' not found in config. Use `sdkm config add-sdk {} --download-url <URL> --bin-dir <DIR>` to register it first.",
                    name, name
                );
            }
        }

        // 按类型校验值
        let ty = field_type(&key);
        let validated = validate_by_type(&self.value, &ty)?;

        // 设置值
        manager.config.set_value(&key, validated)?;

        // 脱敏后的确认输出
        let display_value = mask_by_type(&self.value, &ty);
        success!("{} = {}", key.display(), display_value);
        Ok(())
    }
}

impl ConfigGetHandler {
    fn run(&self) -> Result<()> {
        let manager = SdkManager::new()?;

        // 解析键名
        let key = parse_config_key(&self.key)?;

        // 校验 SDK 键名对应的 SDK 是否存在
        if let ConfigKey::Sdk { name, .. } | ConfigKey::SdkExtraVar { name, .. } | ConfigKey::SdkExtraPath { name, .. } = &key {
            if manager.config.find_sdk_by_name(name).is_err() {
                anyhow::bail!("SDK '{}' not found in config.", name);
            }
        }

        // 获取值（自动脱敏）
        let value = manager.config.get_value(&key)?;
        if value.is_empty() {
            info!("{} = (none)", key.display());
        } else {
            info!("{} = {}", key.display(), value);
        }
        Ok(())
    }
}

fn run_list() -> Result<()> {
    let manager = SdkManager::new()?;
    let entries = manager.config.list_all_values();

    for (key, value) in &entries {
        info!("{} = {}", key, value);
    }
    Ok(())
}

impl ConfigDeleteHandler {
    fn run(&self) -> Result<()> {
        let mut manager = SdkManager::new()?;

        // 解析键名
        let key = parse_config_key(&self.key)?;

        // 校验 SDK 键名对应的 SDK 是否存在
        if let ConfigKey::Sdk { name, .. } | ConfigKey::SdkExtraVar { name, .. } | ConfigKey::SdkExtraPath { name, .. } = &key {
            if manager.config.find_sdk_by_name(name).is_err() {
                anyhow::bail!("SDK '{}' not found in config.", name);
            }
        }

        // 获取元数据（检查是否可删除）
        let meta = manager.config.key_meta_for(&key);
        if !meta.deletable {
            anyhow::bail!(
                "Cannot delete config key '{}'. It is a required field or belongs to a built-in SDK.\nUse `sdkm config set {} <value>` to modify it instead.",
                key.display(),
                key.display()
            );
        }

        // 删除值
        manager.config.delete_value(&key)?;

        success!("{} has been deleted (reset to default: {})", key.display(), meta.default_desc);
        Ok(())
    }
}

fn run_edit() -> Result<()> {
    let config_path = util::path::get_sdkm_config_path()?;

    // 检测编辑器
    let editor = detect_editor();
    info!("Opening config with editor: {}", editor);
    detail!("Config file: {}", config_path.display());

    // 调用编辑器
    let status = Command::new(&editor)
        .arg(&config_path)
        .status()
        .context(format!("Failed to launch editor '{}'", editor))?;

    if !status.success() {
        warning!("Editor exited with non-zero status. Skipping validation.");
        return Ok(());
    }

    // 校验 TOML 格式
    info!("Validating config syntax...");
    match SdkmConfig::read_from_disk() {
        Ok(_) => {
            success!("Config updated successfully.");
        }
        Err(e) => {
            warning!("Config syntax error detected:");
            detail!("{}", e);
            warning!("Please fix the error and re-run `sdkm config edit` to correct it.");
        }
    }

    Ok(())
}

/// Detect system editor: $EDITOR / $VISUAL → platform fallback
fn detect_editor() -> String {
    // 优先使用环境变量
    if let Ok(ed) = std::env::var("EDITOR") {
        if !ed.is_empty() {
            return ed;
        }
    }
    if let Ok(ed) = std::env::var("VISUAL") {
        if !ed.is_empty() {
            return ed;
        }
    }
    // 平台 fallback
    if cfg!(target_os = "windows") {
        "notepad".to_string()
    } else {
        "vi".to_string()
    }
}

impl AddSdkHandler {
    fn run(&self) -> Result<()> {
        let mut manager = SdkManager::new()?;

        // 校验 name 不重名
        if manager.config.sdks.iter().any(|s| s.name == self.name) {
            anyhow::bail!("SDK '{}' already exists in config.", self.name);
        }

        // 校验 download_url（UrlTemplate）
        let download_url_validated = validate_by_type(&self.download_url, &ValueType::UrlTemplate)?;
        let download_url = download_url_validated.into_string();

        // 解析 bin_dir：不传 → 空（二进制在根目录），传了 → 校验禁止路径分隔符
        let bin_dir = match &self.bin_dir {
            Some(dir) => {
                let validated = validate_by_type(dir, &ValueType::FreeString)?;
                validated.into_string()
            }
            None => String::new(), // 空字符串 = 二进制在 SDK 根目录
        };

        // 校验可选 URL 字段
        let version_url = self.version_url.as_ref().map(|u| {
            validate_by_type(u, &ValueType::Url)
                .map(|v: ValidatedValue| v.into_string())
        }).transpose()?;

        let version_fallback_url = self.version_fallback_url.as_ref().map(|u| {
            validate_by_type(u, &ValueType::Url)
                .map(|v: ValidatedValue| v.into_string())
        }).transpose()?;

        let download_fallback_url = self.download_fallback_url.as_ref().map(|u| {
            validate_by_type(u, &ValueType::UrlTemplate)
                .map(|v: ValidatedValue| v.into_string())
        }).transpose()?;

        // 解析 extra_var KEY=VALUE 格式
        let mut extra_vars = HashMap::new();
        for var in &self.extra_var {
            let parts: Vec<&str> = var.splitn(2, '=').collect();
            if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                anyhow::bail!("Invalid extra_var format '{}'. Expected KEY=VALUE.", var);
            }
            extra_vars.insert(parts[0].to_string(), parts[1].to_string());
        }

        // 构造 SdkConfig
        let sdk_config = SdkConfig {
            name: self.name.clone(),
            version_url,
            version_fallback_url,
            download_url,
            download_fallback_url,
            current_version: None,
            bin_dir,
            extra_vars,
            extra_paths: self.extra_path.clone(),
        };

        manager.config.add_sdk(sdk_config)?;

        success!("SDK '{}' added to config.", self.name);
        detail!("Run `sdkm install {} <version>` to start using it.", self.name);
        Ok(())
    }
}

impl RemoveSdkHandler {
    fn run(&self) -> Result<()> {
        let mut manager = SdkManager::new()?;

        manager.config.remove_sdk(&self.name)?;

        success!("SDK '{}' removed from config.", self.name);
        Ok(())
    }
}
