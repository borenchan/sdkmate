use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::clone;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Sdk {
    Built(BuiltinSdk),
    Custom(String),
}
/// builtin sdk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinSdk {
    ///java programming language development environment
    Java,
    /// java programming language package manager
    Maven,
    /// node programming language
    Node,
    /// python programming language
    Python,
}

impl FromStr for Sdk {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match BuiltinSdk::from_str(s) {
            Ok(b) => Sdk::Built(b),
            Err(_) => Sdk::Custom(s.to_string()),
        })
    }
}

impl Display for Sdk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Sdk::Built(b) => b.fmt(f),
            Sdk::Custom(o) => o.fmt(f),
        }
    }
}
impl FromStr for BuiltinSdk {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "java" => Ok(BuiltinSdk::Java),
            "maven" => Ok(BuiltinSdk::Maven),
            "node" => Ok(BuiltinSdk::Node),
            "python" => Ok(BuiltinSdk::Python),
            _ => Err(anyhow::anyhow!("not builtin sdk")),
        }
    }
}
impl Display for BuiltinSdk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinSdk::Java => write!(f, "java"),
            BuiltinSdk::Maven => write!(f, "maven"),
            BuiltinSdk::Node => write!(f, "node"),
            BuiltinSdk::Python => write!(f, "python"),
        }
    }
}
impl BuiltinSdk {
    /// get sdk bin directory
    /// For Python install_only: after double-lift normalization,
    /// Windows has python.exe at root, Unix has python3 in bin/
    pub fn get_sdk_bin_dir(&self) -> &str {
        match self {
            BuiltinSdk::Node => "",
            BuiltinSdk::Python => {
                // install_only 二次提升后：Windows 扁平结构，Unix bin/ 子目录
                if cfg!(target_os = "windows") { "" } else { "bin" }
            }
            _ => "bin",
        }
    }

    /// PATH 冲突检测：返回该 SDK 的主可执行文件名（不含扩展名）
    /// Windows 运行时会自动追加 .exe / .cmd；Unix 使用原始名
    pub fn primary_executables(&self) -> &[&str] {
        match self {
            BuiltinSdk::Java => &["java", "javac"],
            BuiltinSdk::Node => &["node", "npm"],
            BuiltinSdk::Python => &["python", "python3"],
            BuiltinSdk::Maven => &["mvn"],
        }
    }
}
