//! `sdkm hook <shell>` 的输出生成器：委托 `Shell::syntax().generate_hook`（模板在 util::shell_backend）。
//! 纯函数，无 IO、无 stdout 宏（stdout 只吐脚本本身）。

use util::shell::Shell;

/// 生成 hook 脚本（纯函数返字符串，由 CLI 层 println 到 stdout 供 eval/source）
pub fn generate_hook_script(shell: Shell) -> String {
    (shell.syntax().generate_hook)()
}

#[cfg(test)]
mod tests {
    use super::*;
    use util::consts::ENV_HOOK_BASE_PATH;
    use util::shell::Shell;

    #[test]
    fn hook_scripts_reference_env_names() {
        for shell in Shell::ALL {
            let script = generate_hook_script(shell);
            assert!(script.contains(ENV_HOOK_BASE_PATH), "{shell:?} 缺 BASE_PATH 保存");
            assert!(script.contains("sdkm env"), "{shell:?} 缺 env 调用");
            // 每次提示符渲染都触发（无 PWD 比对短路——热更新依赖）
            assert!(!script.contains("LAST_PWD"), "{shell:?} 不应含 LAST_PWD 比对");
        }
    }

    #[test]
    fn hook_scripts_are_ascii_only() {
        // PS 5.1 按 ANSI 代码页（中文系统 GBK）解码原生命令输出，任何非 ASCII 字节都可能被误解码
        // 成破坏脚本的字符；fish/ps 输出同理会被 eval/source。
        for shell in Shell::ALL {
            let script = generate_hook_script(shell);
            assert!(
                script.is_ascii(),
                "{shell:?} hook 输出含非 ASCII 字符（会在 GBK 代码页下破坏脚本）"
            );
        }
    }

    /// 🔴 fish 守卫：必须用 `| source` 消费多行脚本，绝不能用 `eval (`——命令替换按换行拆参 +
    /// eval 空格连接会把多行脚本压成一行（注释吞行/if-end 破坏/多行 set 静默合并）。
    #[test]
    fn fish_hook_uses_pipe_source_not_eval() {
        let script = generate_hook_script(Shell::Fish);
        assert!(script.contains("| source"), "fish hook 必须用 | source");
        assert!(!script.contains("eval ("), "fish hook 禁用 eval ( 消费多行脚本（会被压平）");
        // --on-event fish_prompt：每次提示符渲染触发 = 热更新语义
        assert!(script.contains("--on-event fish_prompt"), "fish hook 必须注册 fish_prompt");
    }
}
