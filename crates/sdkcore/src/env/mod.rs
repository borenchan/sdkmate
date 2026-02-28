use anyhow::Result;
use util::sdk::Sdk;

pub mod windows;

pub trait EnvOperation {
    fn set_sdk_env(&self, sdk: Sdk, sdk_path: &str) -> Result<()>;

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()>;
}
