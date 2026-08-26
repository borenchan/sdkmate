//! Shell 类型与语法后端入口：供 shell hook 注入（init）、hook/env 脚本生成共用。
//!
//! - `Shell` 枚举 + 自动检测/解析（`sdkm hook <shell>` / `sdkm env` 的 `--shell` 参数）
//! - `Shell::syntax()` / `Shell::persistence()`：指向 `shell_backend` 内各 shell 的静态表
//! - `Shell::profile_paths()`：Unix 相对 HOME + PowerShell（Windows）Documents 多 profile
//!
//! 新增 shell：加枚举变体 + `shell_backend/<name>.rs` 填充表，编译期 match 强制完备。

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

pub use crate::shell_backend::{PathModel, ProfilePersistence, ShellSyntax};

/// 目标 shell 类型（hook/env/use --shell 的 `--shell` 参数；clap 解析在 cli 层）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl Shell {
    /// 全部 shell（新增变体务必加进此数组，backend 完备性测试护航）
    pub const ALL: [Shell; 4] = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];

    /// 脚本语法表（env/hook/use --shell 生成能力，4 shell 全量）
    pub const fn syntax(&self) -> &'static crate::shell_backend::ShellSyntax {
        match self {
            Shell::Bash => &crate::shell_backend::bash::SYNTAX,
            Shell::Zsh => &crate::shell_backend::zsh::SYNTAX,
            Shell::Fish => &crate::shell_backend::fish::SYNTAX,
            Shell::PowerShell => &crate::shell_backend::pwsh::SYNTAX,
        }
    }

    /// Unix profile 持久化表（bash/zsh/fish；PowerShell 缺席——Windows 走注册表，返 None）
    pub const fn persistence(&self) -> Option<&'static crate::shell_backend::ProfilePersistence> {
        match self {
            Shell::Bash => Some(&crate::shell_backend::bash::PERSISTENCE),
            Shell::Zsh => Some(&crate::shell_backend::zsh::PERSISTENCE),
            Shell::Fish => Some(&crate::shell_backend::fish::PERSISTENCE),
            Shell::PowerShell => None,
        }
    }

    /// 显示名（init 提示、帮助文本用）
    pub fn display_name(&self) -> &'static str {
        self.syntax().display_name
    }

    /// profile 路径列表：PowerShell → [PS7, PS5.1]（Documents 重定向）；其余 → HOME + 相对路径
    pub fn profile_paths(&self) -> Result<Vec<PathBuf>> {
        match self {
            Shell::PowerShell => powershell_profile_paths(),
            _ => {
                let home = env::var_os("HOME").context("HOME environment variable not set")?;
                Ok(vec![PathBuf::from(home).join(self.syntax().profile_relative_path)])
            }
        }
    }
}

/// 自动检测当前 shell：windows → PowerShell；unix 取 `$SHELL` basename 精确匹配，无匹配兜底 Bash
pub fn detect_shell() -> Shell {
    if cfg!(target_os = "windows") {
        return Shell::PowerShell;
    }
    let shell = env::var("SHELL").unwrap_or_default();
    let basename = Path::new(&shell).file_name().and_then(|n| n.to_str()).unwrap_or("");
    detect_from_basename(basename)
}

/// 纯函数：按 $SHELL basename 判定（mise 同款——basename 精确匹配，误判不了 dash/ash 之类的 contains 陷阱）
pub fn detect_from_basename(basename: &str) -> Shell {
    for s in Shell::ALL {
        let b = s.syntax().detect_basename;
        if !b.is_empty() && basename == b {
            return s;
        }
    }
    // sh/dash/ksh 及一切未知 → bash 兜底
    Shell::Bash
}

/// 解析用户显式指定的 shell 名（支持列表由各 shell 的 parse_names 动态汇总，防 stale）
pub fn parse_shell(s: &str) -> Result<Shell> {
    let lower = s.to_lowercase();
    for sh in Shell::ALL {
        if sh.syntax().parse_names.contains(&lower.as_str()) {
            return Ok(sh);
        }
    }
    let supported: Vec<&str> = Shell::ALL
        .iter()
        .flat_map(|sh| sh.syntax().parse_names.iter().copied())
        .collect();
    anyhow::bail!("unsupported shell `{s}` (supported: {})", supported.join(", "))
}

/// Windows PowerShell profile 路径（PS7 与 PS5.1 两个都要注入——用户日常可能用任一版本）。
///
/// 基于 Documents 重定向后的目录（`Documents\PowerShell\...` 与 `Documents\WindowsPowerShell\...`），
/// 顺序固定：PS7 在前，PS5.1 在后。返回的路径文件可能不存在，调用方负责创建。
#[cfg(windows)]
pub fn powershell_profile_paths() -> Result<Vec<PathBuf>> {
    let docs = windows_documents_dir()?;
    Ok(vec![
        docs.join("PowerShell").join("Microsoft.PowerShell_profile.ps1"),
        docs.join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1"),
    ])
}

/// 非 Windows 无法检测到 PowerShell：防御性空（保证 inject 等处无条件引用此函数跨平台编译通过）
#[cfg(not(windows))]
pub fn powershell_profile_paths() -> Result<Vec<PathBuf>> {
    Ok(Vec::new())
}

/// Windows 用户 Documents 目录（遵循「已知文件夹重定向」）
///
/// PowerShell 的 `$PROFILE` 基于此路径（`Documents\WindowsPowerShell\...`）。
/// 若用户把 Documents 重定向到 D 盘（注册表 User Shell Folders\Personal），
/// 硬编码 `USERPROFILE\Documents` 会注入错文件导致 hook 永不加载。先读注册表，
/// 读不到或非绝对路径回退 `USERPROFILE\Documents`。
#[cfg(windows)]
pub fn windows_documents_dir() -> Result<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    // 注册表重定向（含 %USERPROFILE% 变量，需展开）
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders")
        && let Ok::<String, _>(personal) = key.get_value("Personal")
    {
        let expanded = if personal.contains('%') {
            env::var("USERPROFILE")
                .map(|up| personal.replace("%USERPROFILE%", &up))
                .unwrap_or(personal)
        } else {
            personal
        };
        let path = PathBuf::from(expanded);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    // 回退：USERPROFILE\Documents
    let docs = env::var_os("USERPROFILE").context("USERPROFILE environment variable not set")?;
    Ok(PathBuf::from(docs).join("Documents"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shell_aliases() {
        assert_eq!(parse_shell("bash").unwrap(), Shell::Bash);
        assert_eq!(parse_shell("ZSH").unwrap(), Shell::Zsh);
        assert_eq!(parse_shell("fish").unwrap(), Shell::Fish);
        assert_eq!(parse_shell("pwsh").unwrap(), Shell::PowerShell);
        assert_eq!(parse_shell("powershell").unwrap(), Shell::PowerShell);
        assert!(parse_shell("cmd").is_err());
    }

    #[test]
    fn display_names() {
        assert_eq!(Shell::Bash.display_name(), "bash");
        assert_eq!(Shell::Zsh.display_name(), "zsh");
        assert_eq!(Shell::Fish.display_name(), "fish");
        assert_eq!(Shell::PowerShell.display_name(), "PowerShell");
    }

    /// detect：basename 精确匹配 + 无匹配兜底 Bash（sh/dash/ksh）
    #[test]
    fn detect_from_basename_matching() {
        assert_eq!(detect_from_basename("fish"), Shell::Fish);
        assert_eq!(detect_from_basename("zsh"), Shell::Zsh);
        assert_eq!(detect_from_basename("bash"), Shell::Bash);
        assert_eq!(detect_from_basename("sh"), Shell::Bash);
        assert_eq!(detect_from_basename("dash"), Shell::Bash);
        assert_eq!(detect_from_basename(""), Shell::Bash);
    }

    #[test]
    fn profile_relative_paths_sane() {
        assert_eq!(Shell::Bash.syntax().profile_relative_path, ".bashrc");
        assert_eq!(Shell::Zsh.syntax().profile_relative_path, ".zshrc");
        assert_eq!(Shell::Fish.syntax().profile_relative_path, ".config/fish/config.fish");
        // PowerShell 相对路径为空（走 cfg(windows) 的 Documents 分支）
        assert_eq!(Shell::PowerShell.syntax().profile_relative_path, "");
    }
}