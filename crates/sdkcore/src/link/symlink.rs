use anyhow::{Result, anyhow};
use std::{
    fs,
    path::{Path, PathBuf},
};
use util::info;

/// 读取符号链接目标路径，用于回滚备份
/// 返回 Ok(None) 表示路径不存在或不是符号链接
pub fn read_symlink_target<P: AsRef<Path>>(link_path: &P) -> Result<Option<PathBuf>> {
    let path = link_path.as_ref();
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_symlink() {
        return Ok(None); // 是真实目录/文件，不是符号链接
    }
    let target = fs::read_link(path)?;
    Ok(Some(target))
}

/// 删除符号链接或目录/文件（用于回滚）
pub fn remove_symlink<P: AsRef<Path>>(link_path: &P) -> Result<()> {
    let path = link_path.as_ref();
    if !path.exists() {
        return Ok(()); // 路径不存在，无需删除
    }
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    info!("removed symlink at {}", path.display());
    Ok(())
}

/// Create a symbolic link from `original` to `link`.
pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: &P, link: &Q) -> Result<()> {
    let original_path = original.as_ref();
    let link_path = link.as_ref();
    let link_dir = link_path.display().to_string();
    if !original_path.exists() {
        return Err(anyhow!("original path `{}` does not exist", original_path.display()));
    }
    //when exists link, remove it
    if link_path.exists() {
        if link_path.is_dir() {
            fs::remove_dir_all(link_path)?
        } else {
            fs::remove_file(link_path)?
        };
    }

    //create symlink on os
    #[cfg(unix)]
    std::os::unix::fs::symlink(original, link)?;
    #[cfg(windows)]
    {
        if original_path.is_dir() {
            std::os::windows::fs::symlink_dir(original, link)?;
        } else {
            std::os::windows::fs::symlink_file(original, link)?;
        }
    }
    info!(
        "success create symlink, link path: {} original path: {}",
        link_dir,
        original_path.display()
    );
    Ok(())
}
