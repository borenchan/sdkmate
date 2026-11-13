//! Shell hook 注入：`sdkm init` step 5 的核心逻辑（从 init.rs 抽出，保持 init 只做编排）。
//!
//! 职责：自动检测 shell → 定位 profile 路径 → 去重/升级后追加 hook 注册行。
//! profile 路径解析全部来自 `sdkm_util::shell`（Unix ~/.zshrc|.bashrc、Windows PS7+PS5.1），
//! 不再在各处重复写路径魔法值。失败仅 warning 不阻断 init（hook 是增强，缺了不影响
//! 全局 symlink 机制）。注入结果经 sink 发事件（core 零打印）。

use sdkm_util::consts::HOOK_COMMENT_LINE;
use sdkm_util::shell::{Shell, detect_shell};
use std::fs;
use std::path::{Path, PathBuf};

use crate::report::EventSink;

/// 单个 profile 的 hook 注册状态
#[derive(Debug, Clone)]
pub struct ProfileHookStatus {
    pub path: PathBuf,
    pub registered: bool,
}

/// 当前 shell 的 hook 注册状态
#[derive(Debug, Clone)]
pub struct ShellHookStatus {
    pub shell: Shell,
    pub profiles: Vec<ProfileHookStatus>,
}

impl ShellHookStatus {
    /// 所有 profile 均已注册
    pub fn all_registered(&self) -> bool {
        !self.profiles.is_empty() && self.profiles.iter().all(|p| p.registered)
    }
}

/// 只读检测：当前 shell 的各 profile 是否已注册 sdkm hook
///
/// profile 文件不存在按“未注册”处理；不创建/不修改任何文件。
pub fn check_shell_hook() -> anyhow::Result<ShellHookStatus> {
    let shell = detect_shell();
    let marker = shell.syntax().inject_marker;
    let mut profiles = Vec::new();
    for path in shell.profile_paths()? {
        let registered = fs::read_to_string(&path).map(|c| c.contains(marker)).unwrap_or(false);
        profiles.push(ProfileHookStatus { path, registered });
    }
    Ok(ShellHookStatus { shell, profiles })
}

/// 注入 shell hook（step 5）：自动检测 shell → 定位 profile → 去重追加；结果经 sink 发事件
pub fn inject_shell_hook(sink: &dyn EventSink) -> anyhow::Result<()> {
    let shell = detect_shell();
    match inject_hook_to_profile(shell, sink) {
        Ok(true) => {
            sink.warning("Restart your shell for project-level hooks to take effect.".to_string());
        }
        Ok(false) => {
            sink.info(format!(
                "Shell hook already injected in your {} profile — nothing to do.",
                shell.display_name()
            ));
        }
        Err(e) => {
            sink.warning(format!("Failed to inject shell hook: {e}"));
            sink.detail("Project-level auto switching is optional; `sdkm switch` still works globally.".to_string());
        }
    }
    Ok(())
}

/// 注入 shell hook 并返回明确结果（true=本次写入，false=已存在跳过）
///
/// 与 [`inject_shell_hook`] 的区别：失败直接返回 Err，供 GUI/诊断明确反馈。
pub fn inject_shell_hook_checked(sink: &dyn EventSink) -> anyhow::Result<bool> {
    let shell = detect_shell();
    inject_hook_to_profile(shell, sink)
}

/// 往 shell profile 追加 hook 注册行。返 true=本次写入（任一 profile），false=全部已存在跳过
fn inject_hook_to_profile(shell: Shell, sink: &dyn EventSink) -> anyhow::Result<bool> {
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
        if inject_into_profile_file(&profile_path, &hook_line, shell, sink)? {
            any_written = true;
        }
    }
    Ok(any_written)
}

/// 往单个 profile 文件追加 hook 行（去重 + 旧格式升级）。返 true=本次写入，false=已存在跳过
fn inject_into_profile_file(
    profile_path: &Path,
    hook_line: &str,
    shell: Shell,
    sink: &dyn EventSink,
) -> anyhow::Result<bool> {
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
                sink.detail(format!("Hook line upgraded from legacy form: {}", profile_path.display()));
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
    new_content.push('\n');
    new_content.push_str(HOOK_COMMENT_LINE);
    new_content.push('\n');
    new_content.push_str(hook_line);
    new_content.push('\n');
    fs::write(profile_path, new_content)?;
    sink.detail(format!("Hook injected: {}", profile_path.display()));
    Ok(true)
}
