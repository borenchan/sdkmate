use anyhow::Result;
use util::sdk::Sdk;


pub trait EnvOperation {
    /// set env variable
    fn set_sdk_env(&self, sdk: Sdk, sdk_path: &str) -> Result<()>;

    /// add sdk path to PATH
    fn add_sdk_path(&self, sdk_path: &str) -> Result<()>;

    /// get PATH
    fn get_path(&self) -> Result<String>;
}


#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsEnvOperation as OsEnvOperation;

#[cfg(unix)]
mod unix;