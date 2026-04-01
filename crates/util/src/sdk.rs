use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Sdk {
    Built(BuiltinSdk),
    Custom(String)
}
/// builtin sdk

#[derive(Debug, Clone, Copy,PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinSdk {
    ///java programming language development environment
    Java,
    /// java programming language package manager
    Maven,
    /// node programming language
    Node,
    /// python programming language
    Python,
    /// rust programming language
    Rust,
}

impl FromStr for Sdk {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match BuiltinSdk::from_str(s) {
            Ok(b) => Sdk::Built(b),
            Err(other) => Sdk::Custom(other.to_string()),
        })
    }
}

impl Display for Sdk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Sdk::Built(b) => {b.fmt( f)}
            Sdk::Custom(o) => {o.fmt( f)}
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
            "rust" => Ok(BuiltinSdk::Rust),
            _ => Err(anyhow::anyhow!("not builtin sdk"))
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
            BuiltinSdk::Rust => write!(f, "rust"),
        }
    }
}
impl Sdk{
    /// get sdk bin directory
    pub fn get_sdk_bin_dir(&self, sdk_dir: &PathBuf) -> PathBuf {
        match self {
            Sdk::Built(BuiltinSdk::Node) => sdk_dir.clone(),
            _ => sdk_dir.join("bin")
        }
    }
}