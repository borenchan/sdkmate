use crate::consts::{CONFIG_FILE_NAME, SDKM_LINKS_DIR, SDKM_STORE_DIR};
use anyhow::{Context, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// 检查 sdkm 所在目录是否为专用部署文件夹。
///
/// 只检查路径末尾组件（sdkm.exe 的直接父目录）是否包含 "sdkm"（大小写不敏感）。
/// 根路径如 `C:\` 的 file_name() 返回 None → 视为非专用目录。
pub fn is_sdkm_dedicated_dir(sdkm_home: &Path) -> bool {
    let dir_name = sdkm_home.file_name().and_then(|n| n.to_str()).unwrap_or("");
    dir_name.to_lowercase().contains("sdkm")
}

/// 获取 sdkm home 目录
///
/// 优先读 `SDKM_HOME` 环境变量（参考 rustup 的 `RUSTUP_HOME` 模式）：
/// 既支持便携部署时自定义 home 位置，也便于测试注入临时目录做端到端集成测试。
/// 未设置或为空时回退到「运行中可执行文件的父目录」，保持绿色便携语义不变。
pub fn get_sdkm_home() -> Result<PathBuf> {
    if let Ok(home) = env::var("SDKM_HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home));
    }
    let exe_path = env::current_exe().context("cannot locate sdkm executable")?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .context("cannot determine sdkm home dir")
}

/// 获取已安装 sdks 存储目录
pub fn get_installed_sdks_dir() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    Ok(sdkm_home.join(SDKM_STORE_DIR))
}

/// 获取默认符号链接目录（<sdkm_home>/links）
///
/// 跟随 sdkm home：exe 放哪 links 就在哪，跨平台一致、用户级可写。
pub fn get_default_symlink_dir() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    Ok(sdkm_home.join(SDKM_LINKS_DIR))
}

/// 获取 sdkm 配置文件路径
pub fn get_sdkm_config_path() -> Result<PathBuf> {
    let sdkm_home = get_sdkm_home()?;
    Ok(sdkm_home.join(CONFIG_FILE_NAME))
}

/// 递归计算目录总大小（字节）
///
/// 用 `symlink_metadata` 不跟随符号链接，避免循环递归；符号链接本身不计入大小。
pub fn dir_size(path: &Path) -> Result<u64> {
    let mut total: u64 = 0;
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).with_context(|| format!("read dir {}", dir.display()))? {
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    Ok(total)
}

/// 字节数格式化为人类可读（1024 进制，自动 B/KB/MB/GB/TB，1 位小数）
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} {}", bytes, UNITS[0]);
    }
    let mut size = bytes as f64;
    let mut idx = 0;
    while size >= 1024.0 && idx < UNITS.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }
    format!("{:.1} {}", size, UNITS[idx])
}
