//! Shell hook 注入：`sdkm init` step 5 的核心逻辑（从 init.rs 抽出，保持 init 只做编排）。
//!
//! 职责：自动检测 shell → 定位 profile 路径 → 去重/升级后追加 hook 注册行。
//! profile 路径解析全部来自 `util::shell`（Unix ~/.zshrc|.bashrc、Windows PS7+PS5.1），
//! 不再在各处重复写路径魔法值。失败仅 warning 不阻断 init（hook 是增强，缺了不影响
//! 全局 symlink 机制）。

use std::fs;
use std::path::{Path, PathBuf};
use util::shell::{Shell, detect_shell};
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
    // (profile_path, hook_line) 列表：PowerShell 返回 PS7+PS5.1 两个；Unix 每 shell 一个。
    // 路径来自 Shell::profile_paths、注入行来自 ShellSyntax::inject_hook_line（fish 用 `| source`）。
    let targets: Vec<(PathBuf, String)> = shell
        .profile_paths()?
        .into_iter()
        .map(|p| (p, shell.syntax().inject_hook_line.to_string()))
        .collect();

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
    // 去重检测：注入标记来自 backend（精确到 sdkm hook 调用形式，避免误判用户注释）
    let syntax = shell.syntax();
    let marker = syntax.inject_marker;
    if content.contains(marker) {
        // 旧格式升级（存量用户重跑 init 自愈）：遍历 backend 的 legacy_upgrades 对
        // （bash/zsh/fish 表为空 → 循环 0 次跳过；PowerShell 有 scriptblock/backtick 两对形态）
        for (legacy, new_line) in syntax.legacy_upgrades {
            if content.contains(legacy) {
                let upgraded = content.replace(legacy, new_line);
                fs::write(profile_path, upgraded)?;
                detail!("Hook line upgraded from legacy form: {}", profile_path.display());
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
