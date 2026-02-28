use anyhow::{Result, anyhow};
use std::{fs, path::Path};
use util::terminal;

/// Create a symbolic link from `original` to `link`.
pub fn create_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> Result<()> {
    let original_path = original.as_ref();
    let link_path = link.as_ref();
    let link_dir = link_path.display().to_string();
    if !original_path.exists() {
        return Err(anyhow!("Original path `{}` does not exist", original_path.display()));
    }
    //when exists link, remove it
    if link_path.exists() {
        if link_path.is_dir() {
            fs::remove_dir(link_path)?
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
    terminal::info(format!("Symlink created successfully, link path: {}", link_dir).as_str());
    Ok(())
}
