use anyhow::{Result, anyhow};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

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

/// 删除符号链接或目录/文件（用于回滚）——纯操作函数，动作提示由调用方按需发事件。
/// 返回 true = 实际发生了删除（NotFound 幂等返回 false，调用方勿对幂等场景发 removed 提示）
pub fn remove_symlink<P: AsRef<Path>>(link_path: &P) -> Result<bool> {
    let path = link_path.as_ref();
    // 用 symlink_metadata（不跟随），断链也能检测到；NotFound 视为无需删除
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    if meta.is_symlink() || meta.is_file() {
        fs::remove_file(path).or_else(|_| fs::remove_dir(path))?;
    } else {
        fs::remove_dir_all(path)?;
    }
    Ok(true)
}

/// Create a symbolic link from `original` to `link`——纯操作函数，成功提示由调用方发事件
pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: &P, link: &Q) -> Result<()> {
    let original_path = original.as_ref();
    let link_path = link.as_ref();
    if !original_path.exists() {
        return Err(anyhow!("original path `{}` does not exist", original_path.display()));
    }
    // 链接路径已存在时先删除：用 symlink_metadata（不跟随），断链也能检测到
    if let Ok(meta) = fs::symlink_metadata(link_path) {
        if meta.is_symlink() || meta.is_file() {
            // remove_file 删 file symlink/普通文件；Windows dir symlink 需 remove_dir，fallback
            fs::remove_file(link_path).or_else(|_| fs::remove_dir(link_path))?;
        } else {
            // 真实目录（非 symlink）
            fs::remove_dir_all(link_path)?;
        }
    }

    //create symlink on os
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(original, link)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        if original_path.is_dir() {
            symlink_dir(original, link)?;
        } else {
            symlink_file(original, link)?;
        }
    }
    Ok(())
}
