use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::OnceLock;

/// ------------  PLACEHOLDER start  ------------------
pub const PLACEHOLDER_SDK_DIR: &'static str = "{sdk_dir}";
pub const PLACEHOLDER_SDKM_HOME_DIR: &'static str = "{sdkm_home}";
pub const PLACEHOLDER_SDKS_INSTALL_DIR: &'static str = "{sdks_install_dir}";
pub const PLACEHOLDER_OS: &'static str = "{os}";
pub const PLACEHOLDER_ARCH: &'static str = "{arch}";
pub const PLACEHOLDER_OS_EXT: &'static str = "{ext}";


/// ------------  PLACEHOLDER end  ------------------
static STATIC_VARS: OnceLock<HashMap<&'static str, String>> = OnceLock::new();

pub fn init_static_vars() {
    STATIC_VARS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(PLACEHOLDER_OS,   detect_os());
        m.insert(PLACEHOLDER_ARCH, detect_arch());
        m.insert(PLACEHOLDER_OS_EXT,  detect_ext());
        m
    });
}

fn detect_os() -> String {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos"   => "darwin",
        "linux"   => "linux",
        other     => other,
    }.to_string()
}

fn detect_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64"  => "x64",
        "aarch64" => "arm64",
        "x86"     => "x86",
        other     => other,
    }.to_string()
}

fn detect_ext() -> String {
    if cfg!(target_os = "windows") { "zip" } else { "tar.gz" }.to_string()
}

pub struct TemplateRenderer {
    /// key 直接带花括号，如 "{version}"、"{install_dir}"
    dynamic: HashMap<String, String>,
}

impl TemplateRenderer {
    pub fn new() -> Self {
        init_static_vars();
        Self { dynamic: HashMap::new() }
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
    fn default() -> Self { Self::new() }
}