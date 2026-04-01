use anyhow::{Context, Result};
use std::{env, fs, path::{Path, PathBuf}};
use crate::consts::{CONFIG_FILE_NAME, SDKM_STORE_DIR};

/// 获取 sdkm home 目录
pub fn get_sdkm_home() -> Result<PathBuf> {
    let exe_path = env::current_exe()
        .context("cannot locate sdkm executable")?;
    // current_exe 在某些平台可能返回 symlink，需要 canonicalize 解引用
    // let exe_real = exe_path.canonicalize()
    //     .unwrap_or(exe_path); // canonicalize 失败则用原路径
    exe_path.parent()
        .map(|p| p.to_path_buf())
        .context("cannot determine sdkm home dir")
}
/// 获取已安装sdks 存储目录
pub fn get_installed_sdks_dir() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    let sdkm_store_dir = sdkm_home.join(SDKM_STORE_DIR);
    Ok(sdkm_store_dir)
}

/// 获取 sdkm 配置文件路径
pub fn get_sdkm_config_path() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    let sdkm_config_path = sdkm_home.join(CONFIG_FILE_NAME);
    Ok(sdkm_config_path)
}
