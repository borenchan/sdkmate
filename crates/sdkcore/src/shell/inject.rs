//! Shell hook 注入：`sdkm init` step 5 的核心逻辑（从 init.rs 抽出，保持 init 只做编排）。
//!
//! 职责：自动检测 shell → 定位 profile 路径 → 去重/升级后追加 hook 注册行。
//! profile 路径解析全部来自 `util::shell`（Unix ~/.zshrc|.bashrc、Windows PS7+PS5.1），
//! 不再在各处重复写路径魔法值。失败仅 warning 不阻断 init（hook 是增强，缺了不影响
//! 全局 symlink 机制）。

use std::fs;
use std::path::{Path, PathBuf};
use util::shell::{Shell, detect_shell, powershell_profile_paths, unix_profile_path};
use util::{detail, info, warning};

/// 注入 shell hook（step 5）：自动检测 shell → 定位 profile → 去重追加
pub fn inject_shell_hook() -> anyhow::Result<()> {
    let shell = detect_shell();
    match inject_hook_to_profile(shell) {
        Ok(true) => {
            warning!("Restart your shell for project-level hooks to take effect.");
        }
        Ok(false) => {
            info!(
                "Shell hook already injected in your {} profile — nothing to do.",
                shell.display_name()
            );
        }
        Err(e) => {
            warning!("Failed to inject shell hook: {}", e);
            detail!("Project-level auto switching is optional; `sdkm switch` still works globally.");
        }
    }
    Ok(())
}

/// 往 shell profile 追加 hook 注册行。返 true=本次写入（任一 profile），false=全部已存在跳过
fn inject_hook_to_profile(shell: Shell) -> anyhow::Result<bool> {
    // (profile_path, hook_line) 列表：Windows 同时注入 PS7 与 PS5.1 两个 profile，
    // 用户日常可能用任意一个版本；Unix 只注入一个。
    let targets: Vec<(PathBuf, String)> = match shell {
        Shell::Bash | Shell::Zsh => {
            let path = unix_profile_path()?;
            let sh = if shell == Shell::Zsh { "zsh" } else { "bash" };
            vec![(path, format!("eval \"$(sdkm hook {})\"", sh))]
        }
        Shell::PowerShell => {
            // 必须用 Invoke-Expression：[scriptblock]::Create 作用域隔离，
            // 函数定义留在 scriptblock 内部作用域，profile 执行完即丢失（hook 静默失效）；
            // -join [Environment]::NewLine：PS 5.1 捕获原生命令输出为行数组需 -join 成串，
            // 用常量避免 `n 被二次解释成换行破坏引号配对
            let line = "Invoke-Expression ((sdkm hook powershell) -join [Environment]::NewLine)".to_string();
            let paths = powershell_profile_paths()?;
            // 与 util::shell 返回顺序对齐：PS7 在前、PS5.1 在后（都注入，各自幂等）
            paths.into_iter().map(|p| (p, line.clone())).collect()
        }
    };

    // 逐个注入；任一本次写入 → true（提示重启 shell），全已存在 → false
    let mut any_written = false;
    for (profile_path, hook_line) in targets {
        if inject_into_profile_file(&profile_path, &hook_line, shell)? {
            any_written = true;
        }
    }
    Ok(any_written)
}

/// 往单个 profile 文件追加 hook 行（去重 + 旧格式升级）。返 true=本次写入，false=已存在跳过
fn inject_into_profile_file(profile_path: &Path, hook_line: &str, shell: Shell) -> anyhow::Result<bool> {
    let content = fs::read_to_string(profile_path).unwrap_or_default();
    // 去重检测：注入标记（精确到 sdkm hook 调用形式，避免误判用户注释）
    let marker = match shell {
        Shell::Bash | Shell::Zsh => "sdkm hook",
        Shell::PowerShell => "(sdkm hook",
    };
    if content.contains(marker) {
        // PowerShell 旧格式升级（存量用户重跑 init 自愈），按旧行形态分别替换：
        // ① scriptblock::Create 作用域隔离导致 hook 静默失效
        // ② `-join "`n"` 反引号被二次解释破坏引号配对
        if shell == Shell::PowerShell {
            let legacy_scriptblock = "& ([scriptblock]::Create((sdkm hook powershell)))";
            let legacy_backtick = "Invoke-Expression ((sdkm hook powershell) -join \"`n\")";
            if content.contains(legacy_scriptblock) {
                let upgraded = content.replace(legacy_scriptblock, hook_line);
                fs::write(profile_path, upgraded)?;
                detail!("Hook line upgraded from scriptblock form: {}", profile_path.display());
                return Ok(true);
            }
            if content.contains(legacy_backtick) {
                let upgraded = content.replace(legacy_backtick, hook_line);
                fs::write(profile_path, upgraded)?;
                detail!("Hook line upgraded from backtick-join form: {}", profile_path.display());
                return Ok(true);
            }
        }
        return Ok(false);
    }

    // parent 目录可能不存在（PS profile 的 Documents\PowerShell\）
    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent)?;
    }
    // 追加（保留原内容 + 空行分隔）
    let mut new_content = content;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str("\n# sdkm project-level version hook\n");
    new_content.push_str(hook_line);
    new_content.push('\n');
    fs::write(profile_path, new_content)?;
    detail!("Hook injected: {}", profile_path.display());
    Ok(true)
}
