//! `sdkm hook <shell>` 的输出生成器：纯函数，无 IO、无 stdout 宏（stdout 只吐脚本本身）。
//!
//! 输出定义 `_sdkm_hook` 函数（**每次提示符渲染都触发** `eval "$(sdkm env)"`——热更新
//! 依赖此语义：改 `.sdkm.toml` 后按回车即生效；高频性能靠 sdkm env 的 mtime 缓存承担）
//! + 一次性保存 `_SDKM_BASE_PATH`（防重复）+ 注册到提示符事件（去重）+ 立即触发一次。

use util::consts::ENV_HOOK_BASE_PATH;
use util::shell::Shell;

/// 生成 hook 脚本（纯函数返字符串，由 CLI 层 println 到 stdout 供 eval）
pub fn generate_hook_script(shell: Shell) -> String {
    match shell {
        Shell::Bash => bash_hook(),
        Shell::Zsh => zsh_hook(),
        Shell::PowerShell => powershell_hook(),
    }
}

fn bash_hook() -> String {
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

fn zsh_hook() -> String {
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

fn powershell_hook() -> String {
    let env_base_path = ENV_HOOK_BASE_PATH;
    // IMPORTANT: keep this output ASCII-only. PS 5.1 decodes native-command stdout
    // using the ANSI codepage (GBK on zh-CN); any non-ASCII (e.g. Chinese comments)
    // can mangle into chars like '}' that break the script. Use English only.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_scripts_reference_env_names() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::PowerShell] {
            let script = generate_hook_script(shell);
            assert!(script.contains(ENV_HOOK_BASE_PATH), "{shell:?} 缺 BASE_PATH 保存");
            assert!(script.contains("sdkm env"), "{shell:?} 缺 env 调用");
            // 每次提示符渲染都触发（无 PWD 比对短路——热更新依赖）
            assert!(!script.contains("LAST_PWD"), "{shell:?} 不应含 LAST_PWD 比对");
        }
    }

    #[test]
    fn hook_scripts_are_ascii_only() {
        // PS 5.1 按 ANSI 代码页（中文系统 GBK）解码原生命令输出，任何非 ASCII
        // 字节都可能被误解码成破坏脚本的字符（如 '}'），故 hook 输出必须纯 ASCII
        for shell in [Shell::Bash, Shell::Zsh, Shell::PowerShell] {
            let script = generate_hook_script(shell);
            assert!(
                script.is_ascii(),
                "{shell:?} hook 输出含非 ASCII 字符（会在 GBK 代码页下破坏脚本）"
            );
        }
    }
}
