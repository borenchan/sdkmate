//! bash 语法后端：hook（PROMPT_COMMAND）+ env 语法 fn + profile 持久化（RebuildLine 模型）。
//! zsh 的 env / persist 语法与 bash 完全相同，zsh.rs 直接复用本文件 fn（不复制）。

use super::{PathModel, ProfilePersistence, ShellSyntax};
use crate::consts::ENV_HOOK_BASE_PATH;
use crate::shell::Shell;

/// hook 完整脚本（模板块逐字节搬运自原 bash_hook；每次提示符渲染触发 = 热更新）
fn hook() -> String {
    let env_base_path = ENV_HOOK_BASE_PATH;
    format!(
        r#"# sdkm hook -- bash
# Re-evaluate on every prompt render (hot-reload: editing .sdkm.toml then Enter applies; perf via sdkm env mtime cache)
_sdkm_hook() {{
    eval "$(sdkm env --shell bash)"
}}
# Save startup PATH once (skip if already set, so re-sourcing the profile keeps base)
[ -z "${{{env_base_path}:-}}" ] && export {env_base_path}="$PATH"
# Register into PROMPT_COMMAND (dedup)
case ":${{PROMPT_COMMAND:-}}:" in
    *":_sdkm_hook:"*) ;;
    *) PROMPT_COMMAND="_sdkm_hook${{PROMPT_COMMAND:+:$PROMPT_COMMAND}}" ;;
esac
_sdkm_hook
"#
    )
}

/// env 脚本 base 兜底自愈行：hook 未注入时（手动 eval）先锚定 base = 当前 PATH，防 PATH 被清空
pub(crate) fn base_self_heal_line() -> String {
    format!(
        "[ -z \"${{{b}:-}}\" ] && export {b}=\"$PATH\"",
        b = ENV_HOOK_BASE_PATH
    )
}

/// env 脚本 PATH 重建行：`export PATH="<bins 冒号拼接>:$_SDKM_BASE_PATH"`（无 bins = base 本身，离开项目还原）
pub(crate) fn path_line(bins: &[String]) -> String {
    let path_value = if bins.is_empty() {
        format!("${}", ENV_HOOK_BASE_PATH)
    } else {
        format!("{}:${}", bins.join(":"), ENV_HOOK_BASE_PATH)
    };
    format!("export PATH=\"{}\"", path_value)
}

/// 赋值行（env 脚本 / use --shell / unix.rs 持久化共用）
pub(crate) fn export_line(k: &str, v: &str) -> String {
    format!("export {k}=\"{v}\"")
}

/// 取消行（对不存在变量静默——unset 返回 0，无需守卫）
pub(crate) fn unset_line(k: &str) -> String {
    format!("unset {k}")
}

/// 匹配 profile 现有行的前缀（`export KEY=`）
pub(crate) fn export_prefix(k: &str) -> String {
    format!("export {k}=")
}

/// RebuildLine：从条目列表重建完整 `export PATH="..."` 行（backref=true 时末尾附 `:$PATH` 引用）
pub(crate) fn profile_path_line(entries: &[String], backref: bool) -> String {
    let mut value = entries.join(":");
    if backref {
        value.push_str(":$PATH");
    }
    format!("export PATH=\"{}\"", value)
}

/// RebuildLine：解析 `export PATH="a:b:$PATH"` → (条目, 是否含 $PATH backref)
pub(crate) fn parse_profile_path_line(line: &str) -> (Vec<String>, bool) {
    let rest = line
        .trim()
        .strip_prefix("export PATH=")
        .unwrap_or(line.trim())
        .trim_matches('"');
    let mut entries = Vec::new();
    let mut backref = false;
    for part in rest.split(':') {
        if part == "$PATH" {
            backref = true;
        } else if !part.is_empty() {
            entries.push(part.to_string());
        }
    }
    (entries, backref)
}

/// source 后输出 PATH 的协议（bash 的 $PATH 是冒号分隔串，echo 直接可用）
pub(crate) fn echo_path_cmd() -> String {
    "echo $PATH".to_string()
}

pub const SYNTAX: ShellSyntax = ShellSyntax {
    shell: Shell::Bash,
    parse_names: &["bash"],
    detect_basename: "bash",
    display_name: "bash",
    shell_flag: "bash",
    profile_relative_path: ".bashrc",
    inject_hook_line: "eval \"$(sdkm hook bash)\"",
    inject_marker: "sdkm hook",
    legacy_upgrades: &[],
    generate_hook: hook,
    base_self_heal_line,
    path_line,
    export_line,
    unset_line,
};

pub const PERSISTENCE: ProfilePersistence = ProfilePersistence {
    shell: Shell::Bash,
    shell_command: "bash",
    echo_path_cmd,
    path_model: PathModel::RebuildLine,
    export_prefix,
    profile_path_line: Some(profile_path_line),
    parse_profile_path_line: Some(parse_profile_path_line),
    add_dir_command: None,
};