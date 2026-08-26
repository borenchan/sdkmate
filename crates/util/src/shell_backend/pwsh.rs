//! PowerShell 语法后端：hook（prompt 包装）+ env 语法。**无 PERSISTENCE**——Windows 走注册表，
//! unix.rs 永不接触 PowerShell，`Shell::persistence()` 对 PowerShell 返 None。

use super::{ShellSyntax};
use crate::consts::ENV_HOOK_BASE_PATH;
use crate::shell::Shell;

/// 当前注入行（inject 与 legacy 升级目标共用）
const INVOKE_LINE: &str = "Invoke-Expression ((sdkm hook powershell) -join [Environment]::NewLine)";

/// hook 完整脚本（模板块逐字节搬运自原 powershell_hook，含 ASCII-only 工程注释）。
/// IMPORTANT: keep this output ASCII-only. PS 5.1 decodes native-command stdout using
/// the ANSI codepage (GBK on zh-CN); any non-ASCII can mangle into chars that break the script.
fn hook() -> String {
    let env_base_path = ENV_HOOK_BASE_PATH;
    format!(
        r#"# sdkm hook -- PowerShell
# Re-evaluate on every prompt render (hot-reload: editing .sdkm.toml then Enter applies; perf via sdkm env mtime cache)
function _sdkm_hook {{
    # PS 5.1 captures native-command output as an array of lines; IEX wants one string,
    # so join them back. Use [Environment]::NewLine (a constant, not an escape) - a
    # literal `n would be re-expanded by the outer IEX into a real newline and break
    # the quote pairing (reports "unexpected }}")
    Invoke-Expression ((sdkm env --shell powershell) -join [Environment]::NewLine)
}}
# Save startup PATH once (skip if already set, so re-sourcing the profile keeps base)
if (-not $env:{env_base_path}) {{ $env:{env_base_path} = $env:PATH }}
# Wrap the prompt function (dedup: only wrap when existing prompt has no _sdkm_hook; prompt runs on every render)
$__sdkm_orig_prompt = $function:prompt
if ("$__sdkm_orig_prompt" -notmatch '_sdkm_hook') {{
    $function:prompt = {{ _sdkm_hook; & $__sdkm_orig_prompt }}.ToString()
}}
_sdkm_hook
"#
    )
}

/// env 脚本 base 兜底自愈行：hook 未注入时（手动 IEX）先锚定 base = 当前 PATH，防 PATH 被清空
fn base_self_heal_line() -> String {
    format!(
        "if (-not $env:{b}) {{ $env:{b} = $env:PATH }}",
        b = ENV_HOOK_BASE_PATH
    )
}

/// env 脚本 PATH 重建行：`$env:PATH = "<list>;" + $env:_SDKM_BASE_PATH`（无 bins = base 本身，离开项目还原）
fn path_line(bins: &[String]) -> String {
    let value = if bins.is_empty() {
        format!("$env:{}", ENV_HOOK_BASE_PATH)
    } else {
        format!("\"{};\" + $env:{}", bins.join(";"), ENV_HOOK_BASE_PATH)
    };
    format!("$env:PATH = {value}")
}

/// 赋值行（env 脚本 / use --shell 共用）
fn export_line(k: &str, v: &str) -> String {
    format!("$env:{k} = \"{v}\"")
}

/// 取消行（对不存在变量静默）
fn unset_line(k: &str) -> String {
    format!("Remove-Item Env:{k} -ErrorAction SilentlyContinue")
}

pub const SYNTAX: ShellSyntax = ShellSyntax {
    shell: Shell::PowerShell,
    parse_names: &["powershell", "pwsh"],
    detect_basename: "",
    display_name: "PowerShell",
    shell_flag: "powershell",
    profile_relative_path: "",
    inject_hook_line: INVOKE_LINE,
    inject_marker: "(sdkm hook",
    // 存量用户旧格式自愈（重跑 init 触发）：scriptblock 作用域隔离失效 / backtick 二次解释破坏引号配对
    legacy_upgrades: &[
        ("& ([scriptblock]::Create((sdkm hook powershell)))", INVOKE_LINE),
        ("Invoke-Expression ((sdkm hook powershell) -join \"`n\")", INVOKE_LINE),
    ],
    generate_hook: hook,
    base_self_heal_line,
    path_line,
    export_line,
    unset_line,
};