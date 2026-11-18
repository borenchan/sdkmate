//! Shell hook 注入：`sdkm init` step 5 的核心逻辑（从 init.rs 抽出，保持 init 只做编排）。
//!
//! 职责：自动检测 shell → 定位 profile 路径 → 去重/升级后追加 hook 注册行；
//! 以及 step 2 的托管环境文件 source 行注入（只 source 一行，环境变量本体在 env 文件里）。
//! profile 路径解析全部来自 `sdkm_util::shell`（Unix ~/.zshrc|.bashrc、Windows PS7+PS5.1），
//! 不再在各处重复写路径魔法值。失败仅 warning 不阻断 init（hook 是增强，缺了不影响
//! 全局 symlink 机制）。注入结果经 sink 发事件（core 零打印）。

use sdkm_util::consts::HOOK_COMMENT_LINE;
use sdkm_util::shell::{Shell, detect_shell};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use crate::env::env_file_path;
use crate::report::EventSink;
#[cfg(unix)]
use sdkm_util::consts::ENV_SOURCE_COMMENT_LINE;
#[cfg(unix)]
use std::env;

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

/// init step 2：把托管环境文件的 source 行注入 profile（在 hook 块之前）。
///
/// - 去重：profile 已引用该文件路径（含 `~/` 写法）则跳过——用户手动 source 时不重复注入
/// - 位置：hook 块之前（hook 启动时要能调用 `sdkm env`，而 sdkm 自身在 env 文件的 PATH 里）
/// - 仅 Unix：Windows 环境变量走注册表，没有 env 文件
#[cfg(unix)]
pub fn inject_env_source(sink: &dyn EventSink) -> anyhow::Result<()> {
    let shell = detect_shell();
    if shell.persistence().is_none() {
        return Ok(());
    }
    let env_file = env_file_path()?;
    let env_file_str = env_file.to_string_lossy().to_string();
    let source_line = (shell.syntax().source_env_line)(&env_file_str);
    for profile_path in shell.profile_paths()? {
        if inject_source_into_profile(&profile_path, &source_line, &env_file_str, shell)? {
            sink.detail(format!("Env file sourced in profile: {}", profile_path.display()));
        }
    }
    Ok(())
}

/// Windows：无 env 文件机制（注册表），空实现保持 init 编排跨平台一致
#[cfg(not(unix))]
pub fn inject_env_source(_sink: &dyn EventSink) -> anyhow::Result<()> {
    Ok(())
}

/// self uninstall：从**当前 shell** 的 profile 精确移除 sdkm 注入的 hook 块。
///
/// 与 hook 注入同源（都按 `$SHELL` 检测），多 shell 混用属少数场景，不做跨 shell 清理的债。
/// 精确匹配三形态（宁可漏删不误删，整行 trim 相等才算）：
/// - 固定注释行 `HOOK_COMMENT_LINE`
/// - 该 shell 的 hook 调用行 `inject_hook_line`
/// - legacy 升级表里的旧形态（PowerShell scriptblock/backtick 存量）
///
/// 文件不存在静默跳过；清理结果经 sink 发 detail 事件。
pub fn remove_shell_hook(sink: &dyn EventSink) -> anyhow::Result<()> {
    let shell = detect_shell();
    for profile_path in shell.profile_paths().unwrap_or_default() {
        match remove_hook_from_file(&profile_path, shell) {
            Ok(true) => sink.detail(format!("Hook removed from profile: {}", profile_path.display())),
            Ok(false) => {}
            Err(e) => sink.warning(format!("failed to clean hook in `{}`: {e}", profile_path.display())),
        }
    }
    Ok(())
}

/// 从单个 profile 移除 hook 块；返 true=有移除并已写回
fn remove_hook_from_file(profile_path: &Path, shell: Shell) -> anyhow::Result<bool> {
    let syntax = shell.syntax();
    let content = match fs::read_to_string(profile_path) {
        Ok(c) => c,
        Err(_) => return Ok(false), // 文件不存在：静默跳过
    };
    let mut exact: Vec<&str> = vec![HOOK_COMMENT_LINE, syntax.inject_hook_line];
    for (legacy, _) in syntax.legacy_upgrades {
        exact.push(legacy);
    }
    let kept: Vec<String> = content
        .lines()
        .filter(|l| !exact.contains(&l.trim()))
        .map(String::from)
        .collect();
    if kept.len() == content.lines().count() {
        return Ok(false);
    }
    let mut text = kept.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    fs::write(profile_path, text)?;
    Ok(true)
}

/// profile 是否已引用该 env 文件（同步识别 HOME 下的 `~/...` 写法）
#[cfg(unix)]
fn references_env_file(content: &str, env_file: &str) -> bool {
    if content.contains(env_file) {
        return true;
    }
    match env::var("HOME") {
        Ok(home) => env_file
            .strip_prefix(&home)
            .is_some_and(|rest| content.contains(&format!("~{}", rest))),
        Err(_) => false,
    }
}

/// hook 块的起始行索引：marker 行向上回溯过固定注释（HOOK_COMMENT_LINE），
/// 让新增行落在注释上方而不是注释与 hook 调用行之间（更清晰）
#[cfg(unix)]
fn find_hook_block_start(lines: &[String], shell: Shell) -> Option<usize> {
    let marker = shell.syntax().inject_marker;
    let mut idx = lines.iter().position(|l| l.contains(marker))?;
    while idx > 0 && lines[idx - 1].trim() == HOOK_COMMENT_LINE {
        idx -= 1;
    }
    Some(idx)
}

/// 把 source 块插到 hook 块之前（无 hook 块则追加末尾），并留空行分隔
#[cfg(unix)]
fn insert_before_hook(lines: &mut Vec<String>, block: &[String], shell: Shell) {
    match find_hook_block_start(lines, shell) {
        Some(idx) => {
            let mut insert: Vec<String> = Vec::with_capacity(block.len() + 2);
            if idx > 0 && !lines[idx - 1].trim().is_empty() {
                insert.push(String::new());
            }
            insert.extend_from_slice(block);
            insert.push(String::new());
            lines.splice(idx..idx, insert);
        }
        None => {
            if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.extend_from_slice(block);
        }
    }
}

/// 往单个 profile 写入 source 块；返 true=本次写入，false=已引用跳过
///
/// 先移除旧的 sdkm source 块（env_file 路径变更 / 重复 init 时避免叠加），再按需插入新块。
#[cfg(unix)]
fn inject_source_into_profile(
    profile_path: &Path,
    source_line: &str,
    env_file: &str,
    shell: Shell,
) -> anyhow::Result<bool> {
    let content = fs::read_to_string(profile_path).unwrap_or_default();
    // 已引用新路径（用户手动 source 或此前注入过）→ 不动
    if references_env_file(&content, env_file) {
        return Ok(false);
    }
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    // env_file 变更时清掉旧路径的 sdkm source 块，避免新旧叠加
    strip_env_source_block(&mut lines);
    let block = [ENV_SOURCE_COMMENT_LINE.to_string(), source_line.to_string()];
    insert_before_hook(&mut lines, &block, shell);

    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    fs::write(profile_path, text)?;
    Ok(true)
}

/// 移除 sdkm 注入的 source 块（固定注释行 + 紧随的 source 行）；返 true=移除过
#[cfg(unix)]
fn strip_env_source_block(lines: &mut Vec<String>) -> bool {
    let mut removed = false;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == ENV_SOURCE_COMMENT_LINE {
            lines.remove(i);
            if i < lines.len() && !lines[i].trim_start().starts_with('#') {
                lines.remove(i);
            }
            removed = true;
        } else {
            i += 1;
        }
    }
    removed
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process;
    use std::thread;

    /// 临时唯一 profile 路径（测试不碰真实 profile）
    fn temp_profile() -> PathBuf {
        let dir = env::temp_dir();
        let name = format!("sdkm_inject_test_{}_{:?}.sh", process::id(), thread::current().id());
        dir.join(name)
    }

    /// source 块必须插在 hook 块之前（hook 启动时 sdkm 需已在 PATH）
    #[test]
    fn env_source_inserted_before_hook_block() {
        let path = temp_profile();
        fs::write(
            &path,
            "# user line\n\n# sdkm project-level version hook\neval \"$(sdkm hook zsh)\"\n",
        )
        .unwrap();
        let shell = Shell::Zsh;
        let source_line = (shell.syntax().source_env_line)("/home/u/sdkm/sdkm_env.sh");
        assert!(inject_source_into_profile(&path, &source_line, "/home/u/sdkm/sdkm_env.sh", shell).unwrap());
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let src_idx = lines.iter().position(|l| l.contains("sdkm_env.sh")).unwrap();
        let hook_idx = lines.iter().position(|l| l.contains("sdkm hook zsh")).unwrap();
        assert!(src_idx < hook_idx, "source 行必须在 hook 行之前：{:?}", lines);
        assert!(lines.contains(&"# user line"), "用户内容应保留");
        let _ = fs::remove_file(&path);
    }

    /// 重复注入幂等：profile 已引用路径（含 ~/ 写法）时跳过；env_file 变更时替换旧块
    #[test]
    fn env_source_dedup_and_replace() {
        let home = env::var("HOME").expect("HOME required");
        let env_file = format!("{home}/sdkm/sdkm_env.sh");
        let shell = Shell::Zsh;
        let source_line = (shell.syntax().source_env_line)(&env_file);
        // 用户已用 ~ 写法手动 source 同一文件：不应再注入
        let path = temp_profile();
        fs::write(&path, "# my devrc\nsource ~/sdkm/sdkm_env.sh\n").unwrap();
        assert!(!inject_source_into_profile(&path, &source_line, &env_file, shell).unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().filter(|l| l.contains("sdkm_env.sh")).count(), 1);
        let _ = fs::remove_file(&path);

        // env_file 变更：旧块被替换，不叠加
        let path = temp_profile();
        let old_file = format!("{home}/old/sdkm_env.sh");
        let old_line = (shell.syntax().source_env_line)(&old_file);
        assert!(inject_source_into_profile(&path, &old_line, &old_file, shell).unwrap());
        assert!(inject_source_into_profile(&path, &source_line, &env_file, shell).unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.matches(ENV_SOURCE_COMMENT_LINE).count(), 1, "只保留一份 sdkm 块");
        assert!(content.contains(&env_file));
        assert!(!content.contains(&old_file));
        let _ = fs::remove_file(&path);
    }

    /// hook 块精准移除：只删 sdkm 注入的注释行 + hook 行（含 legacy 形态），用户内容不动
    #[test]
    fn hook_block_removed_exactly() {
        let path = temp_profile();
        fs::write(
            &path,
            "# user line\n# my own hook comment\neval \"$(sdkm hook bash)\"\n# sdkm project-level version hook\neval \"$(sdkm hook bash)\"\n",
        )
        .unwrap();
        assert!(remove_hook_from_file(&path, Shell::Bash).unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("sdkm hook bash"), "sdkm hook 行应被删除：{content:?}");
        assert!(content.contains("# my own hook comment"), "用户自己的行不能删");
        assert!(content.contains("# user line"), "用户内容应保留");
        // 幂等：再跑一次无变化
        assert!(!remove_hook_from_file(&path, Shell::Bash).unwrap());
        let _ = fs::remove_file(&path);

        // fish 形态
        let path = temp_profile();
        fs::write(&path, "fish_add_path --path \"/opt\"\nsdkm hook fish | source\n").unwrap();
        assert!(remove_hook_from_file(&path, Shell::Fish).unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "fish_add_path --path \"/opt\"\n");
        let _ = fs::remove_file(&path);
    }
}
