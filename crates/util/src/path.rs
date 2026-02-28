use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Get the root directory for the SDK Manager.
pub fn get_sdkm_root_dir(language: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    let root = Path::new("C:\\Program Files\\sdkm").join(language);
    #[cfg(not(windows))]
    let root = Path::new("/usr/local/sdkm").join(language);
    fs::create_dir_all(&root)?;
    Ok(root)
}
