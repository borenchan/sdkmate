use anyhow::{Context, Result};
use std::{env, path::{Path, PathBuf}};
use crate::consts::{CONFIG_FILE_NAME, SDKM_STORE_DIR};

/// 检查 sdkm 所在目录是否为专用部署文件夹。
///
/// 只检查路径末尾组件（sdkm.exe 的直接父目录）是否包含 "sdkm"（大小写不敏感）。
/// 根路径如 `C:\` 的 file_name() 返回 None → 视为非专用目录。
pub fn is_sdkm_dedicated_dir(sdkm_home: &Path) -> bool {
    let dir_name = sdkm_home
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    dir_name.to_lowercase().contains("sdkm")
}

/// 获取 sdkm home 目录
pub fn get_sdkm_home() -> Result<PathBuf> {
    let exe_path = env::current_exe()
        .context("cannot locate sdkm executable")?;
    exe_path.parent()
        .map(|p| p.to_path_buf())
        .context("cannot determine sdkm home dir")
}

/// 获取已安装 sdks 存储目录
pub fn get_installed_sdks_dir() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    Ok(sdkm_home.join(SDKM_STORE_DIR))
}

/// 获取 sdkm 配置文件路径
pub fn get_sdkm_config_path() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    Ok(sdkm_home.join(CONFIG_FILE_NAME))
}
