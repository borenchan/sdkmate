use anyhow::Result;
use crossterm::style::Stylize;
use std::{env, fs};
use std::path::{Path, PathBuf};
use util::consts::{SDKM_SYMLINK_DIR, SDKM_ROOT_DIR};
use crate::link::symlink::create_symlink;
use crate::manager::config::SdkmConfig;

/// init sdkm
pub fn init_sdkm() -> Result<()> {
    println!("initializing sdkm...");
    let dir = env::current_dir()?;
    //1. create sdk store root dir
    let root_dir = dir.join(SDKM_ROOT_DIR);
    if !root_dir.exists() {
        fs::create_dir(&root_dir)?
    }
    println!("sdkm root dir: {}", root_dir.display());

    let mut config = SdkmConfig::read_from_disk()?;
    let sdkm_symlink_dir = config.sdkm_symlink_dir.unwrap_or(SDKM_SYMLINK_DIR.to_string());
    //2. create sdkm symlink dir
    create_symlink(root_dir, sdkm_symlink_dir)?;
    println!("{}", "sdkm initialization is successful".green().bold());
    Ok(())
}


