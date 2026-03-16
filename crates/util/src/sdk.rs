use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Sdk {
    Java,
    Maven,
    NodeJs,
    Python,
    Rust,
    Go,
}
