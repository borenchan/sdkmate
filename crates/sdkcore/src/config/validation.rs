// ──────────────────────────────────────────────────────
// 配置值类型校验与脱敏
// ──────────────────────────────────────────────────────

use anyhow::{Context, Result, bail};
use regex_lite::Regex;
use util::consts::SDKM_SYMLINK_DIR;

use super::keys::{ConfigKey, SdkField};

/// 配置值类型——校验逻辑绑定在类型上，新增字段只需声明类型
#[derive(Debug, Clone)]
pub enum ValueType {
    /// 合法 URL（http/https/socks5）→ url::Url::parse()
    Url,
    /// URL 模板（允许 {version} 等占位符）→ 占位符替换为 dummy 后 Url::parse()
    UrlTemplate,
    /// 布尔值 → 接受 true/false/1/0/yes/no/on/off（大小写不敏感）
    Bool,
    /// 正整数，范围 [min, max] → parse::<u32>() + 范围检查
    U32 { min: u32, max: u32 },
    /// 文件系统路径（不要求存在）→ 基本格式检查
    Path,
    /// 敏感字符串（非空，输出时脱敏）
    Token,
    /// 自由字符串（非空）
    NonEmptyString,
    /// 自由字符串（允许空值，如 bin_dir="" 表示二进制在根目录）→ 禁止路径分隔符
    FreeString,
}

/// 校验后的值容器（携带具体类型，用于 set 操作）
#[derive(Debug)]
pub enum ValidatedValue {
    Url(String),
    Bool(bool),
    U32(u32),
    Path(String),
    Token(String),
    NonEmptyString(String),
    FreeString(String),
}

impl ValidatedValue {
    /// 提取内部值作为字符串（所有类型都转为 String）
    pub fn into_string(self) -> String {
        match self {
            ValidatedValue::Url(s) => s,
            ValidatedValue::Bool(b) => b.to_string(),
            ValidatedValue::U32(n) => n.to_string(),
            ValidatedValue::Path(s) => s,
            ValidatedValue::Token(s) => s,
            ValidatedValue::NonEmptyString(s) => s,
            ValidatedValue::FreeString(s) => s,
        }
    }

    /// 获取内部值的字符串引用（仅适用于持有 String 的变体）
    pub fn as_str_ref(&self) -> &str {
        match self {
            ValidatedValue::Url(s) => s,
            ValidatedValue::Bool(_) | ValidatedValue::U32(_) => {
                // Bool/U32 不持有 String，无法返回引用
                unreachable!("use into_string() for Bool/U32 variants")
            }
            ValidatedValue::Path(s) => s,
            ValidatedValue::Token(s) => s,
            ValidatedValue::NonEmptyString(s) => s,
            ValidatedValue::FreeString(s) => s,
        }
    }
}

/// 键名元数据：是否可 delete、值类型、默认值描述
#[derive(Debug)]
pub struct KeyMeta {
    /// 是否允许 delete 操作
    pub deletable: bool,
    /// 字段类型（用于校验 + 脱敏）
    pub value_type: ValueType,
    /// 默认值描述（用于 list 输出和 delete 后提示）
    pub default_desc: String,
}

/// 根据 ConfigKey 返回该字段的 ValueType
/// 新增字段只需在此处声明类型，自动获得对应的校验和脱敏
pub fn field_type(key: &ConfigKey) -> ValueType {
    match key {
        ConfigKey::SymlinkDir => ValueType::Path,
        ConfigKey::NetworkProxy => ValueType::Url,
        ConfigKey::NetworkSslVerify => ValueType::Bool,
        ConfigKey::NetworkConnectTimeout => ValueType::U32 { min: 1, max: 600 },
        ConfigKey::NetworkCacheTtlSecs => ValueType::U32 { min: 0, max: 86400 },
        ConfigKey::NetworkGithubToken => ValueType::Token,
        ConfigKey::Sdk {
            field: SdkField::VersionUrl,
            ..
        } => ValueType::Url,
        ConfigKey::Sdk {
            field: SdkField::VersionFallbackUrl,
            ..
        } => ValueType::Url,
        ConfigKey::Sdk {
            field: SdkField::DownloadUrl,
            ..
        } => ValueType::UrlTemplate,
        ConfigKey::Sdk {
            field: SdkField::DownloadFallbackUrl,
            ..
        } => ValueType::UrlTemplate,
        ConfigKey::Sdk {
            field: SdkField::CurrentVersion,
            ..
        } => ValueType::NonEmptyString,
        ConfigKey::Sdk {
            field: SdkField::BinDir,
            ..
        } => ValueType::FreeString,
        ConfigKey::SdkExtraVar { .. } => ValueType::NonEmptyString,
        ConfigKey::SdkExtraPath { .. } => ValueType::Path,
    }
}

/// 根据 ConfigKey 返回键名元数据（是否可 delete、值类型、默认值描述）
/// 内置 SDK 的所有字段 deletable=false（只允许 set 修改，不允许 delete）
pub fn key_meta(key: &ConfigKey, is_builtin: bool) -> KeyMeta {
    // 内置 SDK 保护：所有字段不可 delete
    let sdk_deletable = !is_builtin;

    match key {
        ConfigKey::SymlinkDir => KeyMeta {
            deletable: false,
            value_type: ValueType::Path,
            default_desc: SDKM_SYMLINK_DIR.to_string(),
        },
        ConfigKey::NetworkProxy => KeyMeta {
            deletable: true,
            value_type: ValueType::Url,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::NetworkSslVerify => KeyMeta {
            deletable: false,
            value_type: ValueType::Bool,
            default_desc: "true".to_string(),
        },
        ConfigKey::NetworkConnectTimeout => KeyMeta {
            deletable: false,
            value_type: ValueType::U32 { min: 1, max: 600 },
            default_desc: "30".to_string(),
        },
        ConfigKey::NetworkCacheTtlSecs => KeyMeta {
            deletable: false,
            value_type: ValueType::U32 { min: 0, max: 86400 },
            default_desc: "3600".to_string(),
        },
        ConfigKey::NetworkGithubToken => KeyMeta {
            deletable: true,
            value_type: ValueType::Token,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::VersionUrl,
            ..
        } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::Url,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::VersionFallbackUrl,
            ..
        } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::Url,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::DownloadUrl,
            ..
        } => KeyMeta {
            deletable: false, // 必须字段，任何 SDK 都不可删除
            value_type: ValueType::UrlTemplate,
            default_desc: "(required)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::DownloadFallbackUrl,
            ..
        } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::UrlTemplate,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::CurrentVersion,
            ..
        } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::NonEmptyString,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::Sdk {
            field: SdkField::BinDir,
            ..
        } => KeyMeta {
            deletable: false, // 必须字段，任何 SDK 都不可删除
            value_type: ValueType::FreeString,
            default_desc: "(empty = binaries in SDK root dir)".to_string(),
        },
        ConfigKey::SdkExtraVar { .. } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::NonEmptyString,
            default_desc: "(none)".to_string(),
        },
        ConfigKey::SdkExtraPath { .. } => KeyMeta {
            deletable: sdk_deletable,
            value_type: ValueType::Path,
            default_desc: "(none)".to_string(),
        },
    }
}

// ──────────────────────────────────────────────────────
// 按类型校验实现
// ──────────────────────────────────────────────────────

/// 根据 ValueType 对原始字符串执行校验 + 类型转换
pub fn validate_by_type(raw: &str, ty: &ValueType) -> Result<ValidatedValue> {
    match ty {
        ValueType::Url => validate_url(raw),
        ValueType::UrlTemplate => validate_url_template(raw),
        ValueType::Bool => validate_bool(raw),
        ValueType::U32 { min, max } => validate_u32(raw, *min, *max),
        ValueType::Path => validate_path(raw),
        ValueType::Token => validate_token(raw),
        ValueType::NonEmptyString => validate_non_empty_string(raw),
        ValueType::FreeString => validate_free_string(raw),
    }
}

/// 根据 ValueType 对显示值进行脱敏
pub fn mask_by_type(display: &str, ty: &ValueType) -> String {
    match ty {
        ValueType::Token => mask_token(display),
        _ => display.to_string(),
    }
}

/// URL 校验：必须是合法 HTTP/HTTPS/SOCKS5 URL
fn validate_url(raw: &str) -> Result<ValidatedValue> {
    let parsed = url::Url::parse(raw).context(format!("Invalid URL: '{}'", raw))?;
    match parsed.scheme() {
        "http" | "https" | "socks5" => Ok(ValidatedValue::Url(raw.to_string())),
        s => bail!("Invalid URL scheme '{}'. Supported: http, https, socks5", s),
    }
}

/// URL 模板校验：含 {version} 等占位符的 URL，先将占位符替换为 dummy 再验证
fn validate_url_template(raw: &str) -> Result<ValidatedValue> {
    // 将所有 {xxx} 占位符替换为合法路径段 dummy
    let dummy_url = replace_placeholders_with_dummy(raw);
    url::Url::parse(&dummy_url).context(format!("Invalid URL template: '{}'", raw))?;
    Ok(ValidatedValue::Url(raw.to_string()))
}

/// 将 {xxx} 占位符替换为合法路径段 dummy，用于 URL 模板校验
fn replace_placeholders_with_dummy(template: &str) -> String {
    let mut result = template.to_string();
    // 匹配 {任意内容} 并替换为 dummy
    let re = Regex::new(r"\{[^}]+\}").unwrap();
    result = re.replace_all(&result, "dummy").to_string();
    result
}

/// 布尔值校验：接受 true/false/1/0/yes/no/on/off（大小写不敏感，与 git 一致）
fn validate_bool(raw: &str) -> Result<ValidatedValue> {
    let lower = raw.to_lowercase();
    match lower.as_str() {
        "true" | "1" | "yes" | "on" => Ok(ValidatedValue::Bool(true)),
        "false" | "0" | "no" | "off" => Ok(ValidatedValue::Bool(false)),
        _ => bail!("Invalid boolean value '{}'. Accepted: true/false/1/0/yes/no/on/off", raw),
    }
}

/// 正整数校验：范围 [min, max]
fn validate_u32(raw: &str, min: u32, max: u32) -> Result<ValidatedValue> {
    let num: u32 = raw.parse().context(format!("Invalid integer: '{}'", raw))?;
    if num < min || num > max {
        bail!("Value {} out of range [{}, {}]", num, min, max);
    }
    Ok(ValidatedValue::U32(num))
}

/// 路径校验：基本格式检查（不要求存在）
fn validate_path(raw: &str) -> Result<ValidatedValue> {
    if raw.is_empty() {
        bail!("Path value cannot be empty");
    }
    Ok(ValidatedValue::Path(raw.to_string()))
}

/// Token 校验：非空字符串
fn validate_token(raw: &str) -> Result<ValidatedValue> {
    if raw.is_empty() {
        bail!("Token value cannot be empty");
    }
    Ok(ValidatedValue::Token(raw.to_string()))
}

/// 非空字符串校验
fn validate_non_empty_string(raw: &str) -> Result<ValidatedValue> {
    if raw.is_empty() {
        bail!("Value cannot be empty");
    }
    Ok(ValidatedValue::NonEmptyString(raw.to_string()))
}

/// 自由字符串校验（允许空值，禁止路径分隔符）
/// 空值表示二进制在 SDK 根目录（如 Node.js、Windows Python）
fn validate_free_string(raw: &str) -> Result<ValidatedValue> {
    if raw.contains('/') || raw.contains('\\') {
        bail!(
            "Value must be a simple directory name, not a path with separators. Use empty string for binaries in SDK root."
        );
    }
    Ok(ValidatedValue::FreeString(raw.to_string()))
}

/// Token 脱敏：只显示前 4 字符 + ***
/// 如 ghp_abc123 → ghp_***
pub fn mask_token(token: &str) -> String {
    if token.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &token[..4])
    }
}
