//! fish 语法后端：hook（--on-event fish_prompt）+ env 语法 + profile 持久化（PerDirCommand 模型）。
//!
//! ## fish 关键语义（务必遵循）
//! - **一律 `| source`，禁 `eval (...)`**：fish 命令替换按换行拆参、eval 用空格连接参数 → 多行
//!   脚本被压成一行（注释吞行、if/end 破坏、多行 set 静默合并成 list）。`SOMECOMMAND | source`
//!   是 starship/zoxide/direnv 的标准 idiom；source 的 local scope 不影响 `set -gx` 全局语义。
//! - **`_SDKM_BASE_PATH` 在 fish 是 list**：`set -gx e $PATH` 存 list，还原 `set -gx PATH $_SDKM_BASE_PATH`
//!   按元素展开（含空格路径天然保留）——base 引用不引号、bin 单独引号。
//! - **`set -e` 对不存在变量报错**：unset 行用 `set -q K; and set -e K` 守卫。
//! - **PATH 持久化用 `fish_add_path --path "<dir>"`**（req fish ≥ 3.2，2021）：默认 --path 改全局
//!   PATH 不写 universal fish_user_paths（语义对齐 bash「config 每次启动执行，删行即移除」）；幂等
//!   由 fish_add_path 保证。

use super::{PathModel, ProfilePersistence, ShellSyntax};
use crate::consts::ENV_HOOK_BASE_PATH;
use crate::shell::Shell;

/// hook 完整脚本：`--on-event fish_prompt` 每次提示符渲染触发；函数重复定义天然覆盖（无需去重拼接）
fn hook() -> String {
    let e = ENV_HOOK_BASE_PATH;
    format!(
        r#"# sdkm hook -- fish
# Re-evaluate on every prompt render (hot-reload: editing .sdkm.toml then Enter applies; perf via sdkm env mtime cache)
function _sdkm_hook --on-event fish_prompt
    sdkm env --shell fish | source
end
# Save startup PATH once (skip if already set, so re-sourcing the profile keeps base)
if not set -q {e}
    set -gx {e} $PATH
end
_sdkm_hook
"#
    )
}

/// env 脚本 base 兜底自愈行：hook 未注入时（手动 source）先锚定 base = 当前 PATH，防 PATH 被清空
fn base_self_heal_line() -> String {
    format!("if not set -q {e}\n    set -gx {e} $PATH\nend", e = ENV_HOOK_BASE_PATH)
}

/// env 脚本 PATH 重建行：`set -gx PATH "bin1" "bin2" $_SDKM_BASE_PATH`（空格分隔、bin 引号、base 不引号）
fn path_line(bins: &[String]) -> String {
    if bins.is_empty() {
        format!("set -gx PATH ${}", ENV_HOOK_BASE_PATH)
    } else {
        let quoted: Vec<String> = bins.iter().map(|b| format!("\"{b}\"")).collect();
        format!("set -gx PATH {} ${}", quoted.join(" "), ENV_HOOK_BASE_PATH)
    }
}

/// 赋值行（env 脚本 / use --shell 共用；持久化 export 用同语法）
fn export_line(k: &str, v: &str) -> String {
    format!("set -gx {k} \"{v}\"")
}

/// 取消行：fish 的 `set -e` 对不存在变量会报错，先 `set -q` 守卫（幂等 unset）
fn unset_line(k: &str) -> String {
    format!("set -q {k}; and set -e {k}")
}

/// 匹配 profile 现有行的前缀（`set -gx KEY `）
fn export_prefix(k: &str) -> String {
    format!("set -gx {k} ")
}

/// PerDirCommand：构造逐行命令（fish_add_path 幂等，add 无需预先去重；remove 按此串精确匹配删行）
fn add_dir_command(dir: &str) -> String {
    format!("fish_add_path --path \"{dir}\"")
}

/// source 后输出 PATH 的协议：fish 的 $PATH 是 list，`echo $PATH` 会每路径一行 → `string join :` 转冒号串
fn echo_path_cmd() -> String {
    "string join : $PATH".to_string()
}

pub const SYNTAX: ShellSyntax = ShellSyntax {
    shell: Shell::Fish,
    parse_names: &["fish"],
    detect_basename: "fish",
    display_name: "fish",
    shell_flag: "fish",
    profile_relative_path: ".config/fish/config.fish",
    inject_hook_line: "sdkm hook fish | source",
    inject_marker: "sdkm hook fish",
    legacy_upgrades: &[],
    generate_hook: hook,
    base_self_heal_line,
    path_line,
    export_line,
    unset_line,
};

pub const PERSISTENCE: ProfilePersistence = ProfilePersistence {
    shell: Shell::Fish,
    shell_command: "fish",
    echo_path_cmd,
    path_model: PathModel::PerDirCommand,
    export_prefix,
    profile_path_line: None,
    parse_profile_path_line: None,
    add_dir_command: Some(add_dir_command),
};
