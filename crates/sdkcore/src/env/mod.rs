use anyhow::Result;
use util::sdk::Sdk;


pub trait EnvOperation {
    fn set_sdk_env(&self, sdk: Sdk, sdk_path: &str) -> Result<()>;

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()>;
    
    fn get_path(&self) -> Result<String>;
}


#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsEnvOperation as OsOperation;

#[cfg(unix)]
mod unix;