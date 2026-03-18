use std::fmt::Display;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Sdk {
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
    /// go programming language
    Go,
}

impl Display for Sdk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sdk::Java => write!(f, "java"),
            Sdk::Maven => write!(f, "maven"),
            Sdk::Node => write!(f, "node"),
            Sdk::Python => write!(f, "python"),
            Sdk::Rust => write!(f, "rust"),
            Sdk::Go => write!(f, "go"),
        }
    }
 }
