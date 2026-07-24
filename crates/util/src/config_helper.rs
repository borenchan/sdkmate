use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::OnceLock;

/// ------------  PLACEHOLDER start  ------------------
pub const PLACEHOLDER_SDK_DIR: &str = "{sdk_dir}";
pub const PLACEHOLDER_SDKM_HOME_DIR: &str = "{sdkm_home}";
pub const PLACEHOLDER_SDKS_INSTALL_DIR: &str = "{sdks_install_dir}";
pub const PLACEHOLDER_OS: &str = "{os}";
pub const PLACEHOLDER_ARCH: &str = "{arch}";
pub const PLACEHOLDER_OS_EXT: &str = "{ext}";
pub const PLACEHOLDER_VERSION: &str = "{version}";
pub const PLACEHOLDER_FEATURE_VERSION: &str = "{feature_version}";
pub const PLACEHOLDER_RELEASE_TAG: &str = "{release_tag}";
pub const PLACEHOLDER_PLATFORM: &str = "{platform}";

/// ------------  OS/ARCH 映射风格  ------------------

/// OS 名称映射风格：不同 SDK 下载源使用不同的 OS 命名约定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsStyle {
    /// windows / darwin / linux（通用、Python、config.toml 默认）
    #[default]
    Default,
    /// win / darwin / linux（Node.js 下载 URL）
    Short,
    /// windows / mac / linux（Java Adoptium API）
    Adoptium,
}

/// ARCH 名称映射风格
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArchStyle {
    /// x64 / arm64 / x86（通用、Node、config.toml 默认）
    #[default]
    Default,
    /// x64 / aarch64 / x86（Java Adoptium API）
    Adoptium,
    /// amd64 / arm64 / win32（Python Windows embed 包）
    Python,
    /// amd64 / arm64 / 386（Go 官方下载命名）
    Go,
}

/// 标准化的 OS 检测函数，按指定风格映射
pub fn detect_os_with(style: OsStyle) -> String {
    match std::env::consts::OS {
        "windows" => match style {
            OsStyle::Default | OsStyle::Adoptium => "windows",
            OsStyle::Short => "win",
        },
        "macos" => match style {
            OsStyle::Default | OsStyle::Short => "darwin",
            OsStyle::Adoptium => "mac",
        },
        "linux" => "linux",
        other => other,
    }
    .to_string()
}

/// 标准化的 ARCH 检测函数，按指定风格映射
pub fn detect_arch_with(style: ArchStyle) -> String {
    match std::env::consts::ARCH {
        "x86_64" => match style {
            ArchStyle::Default | ArchStyle::Adoptium => "x64",
            ArchStyle::Python | ArchStyle::Go => "amd64",
        },
        "aarch64" => match style {
            ArchStyle::Default | ArchStyle::Python | ArchStyle::Go => "arm64",
            ArchStyle::Adoptium => "aarch64",
        },
        "x86" => match style {
            ArchStyle::Default | ArchStyle::Adoptium => "x86",
            ArchStyle::Python => "win32",
            ArchStyle::Go => "386",
        },
        other => other,
    }
    .to_string()
}

/// 检测当前平台的默认压缩包扩展名
pub fn detect_ext() -> String {
    if cfg!(target_os = "windows") { "zip" } else { "tar.gz" }.to_string()
}

/// 检测 python-build-standalone 平台三元组（如 x86_64-pc-windows-msvc）
/// 用于 Python 下载 URL 的 {platform} 占位符
pub fn detect_platform_triple() -> Result<String> {
    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86") => "i686-pc-windows-msvc",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        (os, arch) => bail!("Unsupported platform for python-build-standalone: os={}, arch={}", os, arch),
    };
    Ok(triple.to_string())
}

/// ------------  静态变量缓存（TemplateRenderer 使用）  ------------------
static STATIC_VARS: OnceLock<HashMap<&'static str, String>> = OnceLock::new();

pub fn init_static_vars() {
    STATIC_VARS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(PLACEHOLDER_OS, detect_os_with(OsStyle::Default));
        m.insert(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Default));
        m.insert(PLACEHOLDER_OS_EXT, detect_ext());
        m
    });
}

pub struct TemplateRenderer {
    /// key 直接带花括号，如 "{version}"、"{install_dir}"
    dynamic: HashMap<String, String>,
}

impl TemplateRenderer {
    pub fn new() -> Self {
        init_static_vars();
        Self {
            dynamic: HashMap::new(),
        }
    }

    pub fn var(mut self, key: &str, value: impl Into<String>) -> Self {
        self.dynamic.insert(key.to_string(), value.into());
        self
    }

    pub fn vars(mut self, pairs: &HashMap<&str, &str>) -> Self {
        for (k, v) in pairs {
            self.dynamic.insert(k.to_string(), v.to_string());
        }
        self
    }

    /// 严格模式：有未解析的 {key} 报错
    pub fn render(&self, template: &str) -> Result<String> {
        let result = self.render_loose(template);
        // 检查是否还有未替换的占位符
        if let Some(start) = result.find('{') {
            if result[start..].contains('}') {
                bail!("unresolved variables in: \"{}\"", result);
            }
        }
        Ok(result)
    }

    /// 宽松模式：未解析的变量保留原样
    pub fn render_loose(&self, template: &str) -> String {
        let mut result = template.to_string();
        // 先替换动态变量（优先级高）
        for (k, v) in &self.dynamic {
            result = result.replace(k.as_str(), v);
        }
        // 再替换静态变量
        if let Some(statics) = STATIC_VARS.get() {
            for (k, v) in statics {
                result = result.replace(*k, v);
            }
        }
        result
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}
