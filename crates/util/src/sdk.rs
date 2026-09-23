use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Sdk {
    Built(BuiltinSdk),
    Custom(String),
}
/// builtin sdk（专属路径 SDK：Java 两步查询、Node v 前缀、Python 双源等）
/// 引擎型内置 SDK（bun/pnpm 等）不在枚举中，以 Sdk::Custom 形态由 builtin::SDK_SEEDS 种子驱动
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
    /// go programming language
    Go,
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            "go" => Ok(BuiltinSdk::Go),
            _ => Err(anyhow::anyhow!("not builtin sdk")),
        }
    }
}
impl Display for BuiltinSdk {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinSdk::Java => write!(f, "java"),
            BuiltinSdk::Maven => write!(f, "maven"),
            BuiltinSdk::Node => write!(f, "node"),
            BuiltinSdk::Python => write!(f, "python"),
            BuiltinSdk::Go => write!(f, "go"),
        }
    }
}
