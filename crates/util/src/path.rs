use anyhow::Result;
use std::{env, fs, path::{Path, PathBuf}};
use crate::consts::SDKM_ROOT_DIR;

/// Get the dir for the SDK Manager's SDKs installed.
pub fn get_installed_sdks_dir() -> Result<PathBuf> {
    let root_dir = env::current_dir()?;
    let sdks_dir = root_dir.join(SDKM_ROOT_DIR);
    Ok(sdks_dir)
}
