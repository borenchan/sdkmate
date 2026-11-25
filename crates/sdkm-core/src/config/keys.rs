// ──────────────────────────────────────────────────────
// 配置键名解析与类型枚举
// ──────────────────────────────────────────────────────

use anyhow::{Context, Result, bail};

/// 配置键的解析结果——将点分隔字符串映射到具体的配置路径
#[derive(Debug, Clone)]
pub enum ConfigKey {
    // 顶层键
    SymlinkDir,
    EnvFile,
    // network 子表键
    NetworkProxy,
    NetworkSslVerify,
    NetworkConnectTimeout,
    NetworkCacheTtlSecs,
    NetworkGithubToken,
    NetworkGithubDownloadMirror,
    // SDK 子表键（需要 SDK name 定位具体条目）
    Sdk { name: String, field: SdkField },
    // SDK extra_vars 单键操作
    SdkExtraVar { name: String, var_key: String },
    // SDK extra_paths 按索引操作
    SdkExtraPath { name: String, index: usize },
}

impl ConfigKey {
    /// 返回键名的点分隔字符串表示（用于显示）
    pub fn display(&self) -> String {
        match self {
            ConfigKey::SymlinkDir => "symlink_dir".to_string(),
            ConfigKey::EnvFile => "env_file".to_string(),
            ConfigKey::NetworkProxy => "network.proxy".to_string(),
            ConfigKey::NetworkSslVerify => "network.ssl_verify".to_string(),
            ConfigKey::NetworkConnectTimeout => "network.connect_timeout".to_string(),
            ConfigKey::NetworkCacheTtlSecs => "network.cache_ttl_secs".to_string(),
            ConfigKey::NetworkGithubToken => "network.github_token".to_string(),
            ConfigKey::NetworkGithubDownloadMirror => "network.github_download_mirror".to_string(),
            ConfigKey::Sdk { name, field } => {
                let field_name = match field {
                    SdkField::VersionUrl => "version_url",
                    SdkField::VersionFallbackUrl => "version_fallback_url",
                    SdkField::DownloadUrl => "download_url",
                    SdkField::DownloadFallbackUrl => "download_fallback_url",
                    SdkField::CurrentVersion => "current_version",
                    SdkField::BinDir => "bin_dir",
                    SdkField::OsStyle => "os_style",
                    SdkField::ArchStyle => "arch_style",
                    SdkField::AddToPath => "add_to_path",
                };
                format!("sdk.{}.{}", name, field_name)
            }
            ConfigKey::SdkExtraVar { name, var_key } => {
                format!("sdk.{}.extra_vars.{}", name, var_key)
            }
            ConfigKey::SdkExtraPath { name, index } => {
                format!("sdk.{}.extra_paths.{}", name, index)
            }
        }
    }
}

/// SDK 子表的字段枚举
#[derive(Debug, Clone)]
pub enum SdkField {
    VersionUrl,
    VersionFallbackUrl,
    DownloadUrl,
    DownloadFallbackUrl,
    CurrentVersion,
    BinDir,
    OsStyle,
    ArchStyle,
    AddToPath,
}

/// 从点分隔字符串解析为 ConfigKey
///
/// 无效键名返回错误（含已知键名清单提示）
/// 格式示例: "network.proxy" → ConfigKey::NetworkProxy
///           "sdk.java.download_url" → ConfigKey::Sdk { name: "java", field: SdkField::DownloadUrl }
///           "sdk.java.extra_vars.JAVA_HOME" → ConfigKey::SdkExtraVar { name: "java", var_key: "JAVA_HOME" }
///           "sdk.java.extra_paths.0" → ConfigKey::SdkExtraPath { name: "java", index: 0 }
pub fn parse_config_key(key: &str) -> Result<ConfigKey> {
    let parts: Vec<&str> = key.split('.').collect();

    match parts.len() {
        // 单段键名：顶层字段
        1 => match parts[0] {
            "symlink_dir" => Ok(ConfigKey::SymlinkDir),
            "env_file" => Ok(ConfigKey::EnvFile),
            _ => bail_invalid_key(key),
        },

        // 两段键名：network.* 或 sdk.*
        2 => match parts[0] {
            "network" => match parts[1] {
                "proxy" => Ok(ConfigKey::NetworkProxy),
                "ssl_verify" => Ok(ConfigKey::NetworkSslVerify),
                "connect_timeout" => Ok(ConfigKey::NetworkConnectTimeout),
                "cache_ttl_secs" => Ok(ConfigKey::NetworkCacheTtlSecs),
                "github_token" => Ok(ConfigKey::NetworkGithubToken),
                "github_download_mirror" => Ok(ConfigKey::NetworkGithubDownloadMirror),
                _ => bail_invalid_key(key),
            },
            "sdk" => bail!(
                "Invalid config key '{}'. SDK keys require 3+ segments: sdk.<name>.<field>\nValid examples: sdk.java.download_url, sdk.java.extra_vars.JAVA_HOME",
                key
            ),
            _ => bail_invalid_key(key),
        },

        // 三段键名：sdk.<name>.<field>
        3 => match parts[0] {
            "sdk" => match parts[2] {
                "version_url" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::VersionUrl,
                }),
                "version_fallback_url" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::VersionFallbackUrl,
                }),
                "download_url" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::DownloadUrl,
                }),
                "download_fallback_url" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::DownloadFallbackUrl,
                }),
                "current_version" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::CurrentVersion,
                }),
                "bin_dir" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::BinDir,
                }),
                "os_style" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::OsStyle,
                }),
                "arch_style" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::ArchStyle,
                }),
                "add_to_path" => Ok(ConfigKey::Sdk {
                    name: parts[1].to_string(),
                    field: SdkField::AddToPath,
                }),
                "extra_vars" | "extra_paths" => bail!(
                    "Invalid config key '{}'. For extra_vars, use: sdk.{}.extra_vars.<KEY>\nFor extra_paths, use: sdk.{}.extra_paths.<N>",
                    key,
                    parts[1],
                    parts[1]
                ),
                _ => bail_invalid_key(key),
            },
            _ => bail_invalid_key(key),
        },

        // 四段键名：sdk.<name>.extra_vars.<KEY> 或 sdk.<name>.extra_paths.<N>
        4 => match parts[0] {
            "sdk" => match parts[2] {
                "extra_vars" => Ok(ConfigKey::SdkExtraVar {
                    name: parts[1].to_string(),
                    var_key: parts[3].to_string(),
                }),
                "extra_paths" => {
                    let index: usize = parts[3].parse().context(format!(
                        "Invalid extra_paths index '{}', must be a non-negative integer",
                        parts[3]
                    ))?;
                    Ok(ConfigKey::SdkExtraPath {
                        name: parts[1].to_string(),
                        index,
                    })
                }
                _ => bail_invalid_key(key),
            },
            _ => bail_invalid_key(key),
        },

        _ => bail_invalid_key(key),
    }
}

/// `sdkm config reset` 的目标
///
/// 与 `ConfigKey` 的差异：支持整组（network）与整条 SDK 恢复，
/// 且只允许有内置默认值的对象（自定义 SDK 无种子默认）。
#[derive(Debug, Clone)]
pub enum ResetTarget {
    /// network 全部设置（github_token 是凭据，不在其列）
    Network,
    /// 单个 network 键
    NetworkKey(ConfigKey),
    /// 全部内置 SDK 恢复种子默认（无种子的自定义 SDK 跳过；保留 current_version）
    Sdks,
    /// 整条内置 SDK 恢复种子默认（保留 current_version）
    Sdk(String),
    /// 单个内置 SDK 字段恢复种子默认
    SdkField { name: String, field: SdkField },
}

/// 解析 reset 目标字符串
///
/// 支持：all | network | network.<key> | sdk.<name> | sdk.<name>.<field>
pub fn parse_reset_target(raw: &str) -> Result<ResetTarget> {
    let target = raw.trim();
    if target == "network" {
        return Ok(ResetTarget::Network);
    }
    if target == "sdks" {
        return Ok(ResetTarget::Sdks);
    }
    let parts: Vec<&str> = target.split('.').collect();
    // sdk.<name>：两段（parse_config_key 要求三段，故单独处理）
    if parts.len() == 2 && parts[0] == "sdk" {
        if parts[1].is_empty() {
            return bail_reset_target(target);
        }
        return Ok(ResetTarget::Sdk(parts[1].to_string()));
    }
    if parts.len() >= 2 && parts[0] == "network" {
        return Ok(ResetTarget::NetworkKey(parse_config_key(target)?));
    }
    if parts.len() >= 3 && parts[0] == "sdk" {
        return match parse_config_key(target)? {
            ConfigKey::Sdk { name, field } => Ok(ResetTarget::SdkField { name, field }),
            _ => bail_reset_target(target),
        };
    }
    bail_reset_target(target)
}

/// reset 目标无效时的统一提示
fn bail_reset_target(target: &str) -> Result<ResetTarget> {
    bail!(
        "Cannot reset '{}'.\nReset targets:\n  network                     all network settings (proxy/ssl/timeout/cache/mirror)\n  network.<key>               a single network setting, e.g. network.proxy\n  sdks                        every built-in SDK (custom SDKs skipped, current_version kept)\n  sdk.<name>                  a built-in SDK's full config (current_version kept)\n  sdk.<name>.<field>          a single built-in SDK field, e.g. sdk.maven.download_url",
        target
    )
}

/// 生成无效键名错误，附带所有合法键名清单
fn bail_invalid_key(key: &str) -> Result<ConfigKey> {
    bail!(
        "Invalid config key '{}'.\nValid keys:\n  {}\n  network.proxy, network.ssl_verify, network.connect_timeout, network.cache_ttl_secs, network.github_token, network.github_download_mirror\n  sdk.<name>.version_url, sdk.<name>.download_url, sdk.<name>.current_version, ...\n  sdk.<name>.extra_vars.<KEY>, sdk.<name>.extra_paths.<N>",
        key,
        "symlink_dir, env_file,"
    )
}

/// 返回所有已知键名列表（用于帮助信息和错误提示）
pub fn known_keys() -> Vec<&'static str> {
    vec![
        "symlink_dir",
        "env_file",
        "network.proxy",
        "network.ssl_verify",
        "network.connect_timeout",
        "network.cache_ttl_secs",
        "network.github_token",
        "network.github_download_mirror",
        "sdk.<name>.version_url",
        "sdk.<name>.version_fallback_url",
        "sdk.<name>.download_url",
        "sdk.<name>.download_fallback_url",
        "sdk.<name>.current_version",
        "sdk.<name>.bin_dir",
        "sdk.<name>.os_style",
        "sdk.<name>.arch_style",
        "sdk.<name>.add_to_path",
        "sdk.<name>.extra_vars.<KEY>",
        "sdk.<name>.extra_paths.<N>",
    ]
}
