//! zsh 语法后端：hook 用 precmd（add-zsh-hook），env / persist 语法与 bash 完全相同（复用 bash::，不复制）。

use super::bash;
use super::{PathModel, ProfilePersistence, ShellSyntax};
use crate::consts::ENV_HOOK_BASE_PATH;
use crate::shell::Shell;

/// hook 完整脚本（模板块逐字节搬运自原 zsh_hook；precmd 每次提示符渲染前触发）
fn hook() -> String {
    let env_base_path = ENV_HOOK_BASE_PATH;
    format!(
        r#"# sdkm hook -- zsh
# Re-evaluate on every prompt render (hot-reload: editing .sdkm.toml then Enter applies; perf via sdkm env mtime cache)
_sdkm_hook() {{
    eval "$(sdkm env --shell zsh)"
}}
# Save startup PATH once (skip if already set, so re-sourcing the profile keeps base)
[ -z "${{{env_base_path}:-}}" ] && export {env_base_path}="$PATH"
# Register into precmd (runs before every prompt render; add-zsh-hook dedups)
autoload -Uz add-zsh-hook
add-zsh-hook precmd _sdkm_hook
_sdkm_hook
"#
    )
}

pub const SYNTAX: ShellSyntax = ShellSyntax {
    shell: Shell::Zsh,
    parse_names: &["zsh"],
    detect_basename: "zsh",
    display_name: "zsh",
    shell_flag: "zsh",
    profile_relative_path: ".zshrc",
    inject_hook_line: "eval \"$(sdkm hook zsh)\"",
    inject_marker: "sdkm hook",
    legacy_upgrades: &[],
    generate_hook: hook,
    base_self_heal_line: bash::base_self_heal_line,
    path_line: bash::path_line,
    export_line: bash::export_line,
    unset_line: bash::unset_line,
};

pub const PERSISTENCE: ProfilePersistence = ProfilePersistence {
    shell: Shell::Zsh,
    shell_command: "zsh",
    echo_path_cmd: bash::echo_path_cmd,
    path_model: PathModel::RebuildLine,
    export_prefix: bash::export_prefix,
    profile_path_line: Some(bash::profile_path_line),
    parse_profile_path_line: Some(bash::parse_profile_path_line),
    add_dir_command: None,
};