//! Shell 类型与 profile 定位：供 shell hook 注入（init）、hook/env 脚本生成共用。
//!
//! 通用无依赖逻辑收敛在此，避免 init.rs / env/unix.rs / hook_script.rs 各自重复
//! 写 profile 路径魔法值。职责：
//! - `Shell` 枚举 + 自动检测/解析（`sdkm hook <shell>` / `sdkm env` 的 `--shell` 参数）
//! - Unix profile 路径（`~/.zshrc` 或 `~/.bashrc`，按 `$SHELL` 判定）
//! - Windows PowerShell profile 路径（PS7 + PS5.1，遵循 Documents 重定向）

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

/// 目标 shell 类型（hook/env/use --shell 的 `--shell` 参数；clap 解析在 cli 层）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    PowerShell,
}

impl Shell {
    /// 显示名（init 提示、帮助文本用）
    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::PowerShell => "PowerShell",
        }
    }
}

/// 自动检测当前 shell：unix 看 `$SHELL` 含 zsh → Zsh 否则 Bash；windows 默认 PowerShell
pub fn detect_shell() -> Shell {
    if cfg!(target_os = "windows") {
        return Shell::PowerShell;
    }
    match env::var("SHELL") {
        Ok(s) if s.contains("zsh") => Shell::Zsh,
        _ => Shell::Bash,
    }
}

/// 解析用户显式指定的 shell 名（`"bash"|"zsh"|"powershell"|"pwsh"`）
pub fn parse_shell(s: &str) -> Result<Shell> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "powershell" | "pwsh" => Ok(Shell::PowerShell),
        other => anyhow::bail!("unsupported shell `{}` (supported: bash, zsh, powershell)", other),
    }
}

/// Unix shell profile 路径：`$SHELL` 含 zsh → `~/.zshrc`，其余 → `~/.bashrc`
///
/// 与 `detect_shell` 同源判定（都看 `$SHELL`）。init 注入 hook 与 env/unix.rs 改
/// profile 共用此函数——不要在各处重新拼路径。
pub fn unix_profile_path() -> Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME environment variable not set")?;
    let profile = if env::var("SHELL").unwrap_or_default().contains("zsh") {
        ".zshrc"
    } else {
        ".bashrc"
    };
    Ok(PathBuf::from(home).join(profile))
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
        assert_eq!(parse_shell("pwsh").unwrap(), Shell::PowerShell);
        assert!(parse_shell("cmd").is_err());
    }

    #[test]
    fn display_names() {
        assert_eq!(Shell::Bash.display_name(), "bash");
        assert_eq!(Shell::Zsh.display_name(), "zsh");
        assert_eq!(Shell::PowerShell.display_name(), "PowerShell");
    }
}
