//! Shell 语法后端：把「每种 shell 的脚本语法与持久化行为」收敛为静态数据表。
//!
//! 两张表：
//! - [`ShellSyntax`]：env/hook/use --shell 的脚本生成能力（**4 shell 全量**，`Shell::syntax()`）
//! - [`ProfilePersistence`]：unix.rs 往 profile 持久化 PATH/export 的能力（**bash/zsh/fish 三行；
//!   PowerShell 缺席**——Windows 走注册表，`Shell::persistence()` 返 None，能力缺席用缺席表达）
//!
//! 每 shell 一个文件（bash.rs / zsh.rs / fish.rs / pwsh.rs），各自的语法 fn + 表行内聚。
//! 新增 shell：加 `Shell` 枚举变体 + 一个文件填充 `SYNTAX` 即可（可只填 syntax 不填 persistence，
//! 表示该 shell 只支持脚本不支持 profile 编辑，天然 Opt-out）。

use crate::shell::Shell;

/// PATH 持久化模型：bash/zsh 单行重建 vs fish 逐行 fish_add_path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathModel {
    /// bash/zsh：`export PATH="a:b:$PATH"` 单行整体重建（读旧行→前置/过滤→整行重写）
    RebuildLine,
    /// fish：每目录一行 `fish_add_path --path "<dir>"`（幂等由 fish_add_path 保证，移除精确删行）
    PerDirCommand,
}

/// 脚本语法表（env/hook/use --shell 生成能力，4 shell 全量）
#[derive(Debug, Clone, Copy)]
pub struct ShellSyntax {
    /// 所属 shell
    pub shell: Shell,
    /// parse_shell 接受名（PowerShell: ["powershell","pwsh"]）
    pub parse_names: &'static [&'static str],
    /// `$SHELL` basename 精确匹配用（"bash"/"zsh"/"fish"；PowerShell 为 "" 不参与 unix detect）
    pub detect_basename: &'static str,
    /// 显示名（init 提示、帮助文本）
    pub display_name: &'static str,
    /// hook / `sdkm env --shell <flag>` 用
    pub shell_flag: &'static str,
    /// 相对 HOME 的 profile 路径（.bashrc / .zshrc / .config/fish/config.fish / PS 为 ""）
    pub profile_relative_path: &'static str,
    /// init 追加到 profile 的 hook 调用行（bash/zsh `eval "$(sdkm hook x)"`、fish `sdkm hook fish | source`、PS `Invoke-Expression ...`）
    pub inject_hook_line: &'static str,
    /// profile 去重 marker（各 shell 在自己 profile 内唯一；fish 精确到 shell 名，避开 bash 的 "sdkm hook" 子串误匹配）
    pub inject_marker: &'static str,
    /// 旧格式升级对 [(旧形态, 新形态)]（PowerShell scriptblock/backtick 存量自愈；其余 shell 空）
    pub legacy_upgrades: &'static [(&'static str, &'static str)],
    /// hook 完整脚本（每次提示符渲染触发；模板块逐字节搬运）
    pub generate_hook: fn() -> String,
    /// env 脚本 base 兜底自愈行（防 hook 未注入时手动 eval/source 清空 PATH）
    pub base_self_heal_line: fn() -> String,
    /// env 脚本 PATH 重建行（引用 _SDKM_BASE_PATH）
    pub path_line: fn(&[String]) -> String,
    /// env / use --shell 赋值行（bash `export K="v"` / fish `set -gx K "v"` / PS `$env:K = "v"`）
    pub export_line: fn(&str, &str) -> String,
    /// env 脚本取消行（bash `unset K` / fish `set -q K; and set -e K` / PS `$env:K = $null`）
    pub unset_line: fn(&str) -> String,
}

/// 持久化表（unix.rs 专用，bash/zsh/fish 三行；PowerShell 缺席——Windows 走注册表）
#[derive(Debug, Clone, Copy)]
pub struct ProfilePersistence {
    /// 所属 shell
    pub shell: Shell,
    /// source 用子 shell 命令（bash/zsh/fish）
    pub shell_command: &'static str,
    /// source 后输出 PATH 的协议命令（bash/zsh `echo $PATH` / fish `string join : $PATH`——fish 的 $PATH 是 list，echo 会每路径一行）
    pub echo_path_cmd: fn() -> String,
    /// PATH 持久化模型（决定 add/remove 走哪套语法）
    pub path_model: PathModel,
    /// 匹配 profile 现有行的前缀（bash `export K=` / fish `set -gx K `）
    pub export_prefix: fn(&str) -> String,
    /// RebuildLine 模型：从条目列表重建完整 PATH 行（含前缀与引号；backref=true 时末尾附 `:$PATH`）
    pub profile_path_line: Option<BuildProfilePathLine>,
    /// RebuildLine 模型：解析 PATH 行 → (条目, 是否含 $PATH backref)
    pub parse_profile_path_line: Option<ParseProfilePathLine>,
    /// PerDirCommand 模型：构造逐行追加/删除命令（fish `fish_add_path --path "<dir>"`）
    pub add_dir_command: Option<fn(&str) -> String>,
}

/// RebuildLine 模型：从条目列表重建完整 PATH 行（backref=true 时末尾附 `:$PATH`）
pub type BuildProfilePathLine = fn(&[String], bool) -> String;
/// RebuildLine 模型：解析 PATH 行 → (条目, 是否含 $PATH backref)
pub type ParseProfilePathLine = fn(&str) -> (Vec<String>, bool);

pub mod bash;
pub mod fish;
pub mod pwsh;
pub mod zsh;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::ENV_HOOK_BASE_PATH;

    /// 完备性守卫：任一 shell 漏填表 → 这里红（加新 shell 忘补表的第一道防线）
    #[test]
    fn backend_table_completeness() {
        for sh in Shell::ALL {
            let s = sh.syntax();
            assert_eq!(s.shell, sh);
            assert!(!s.parse_names.is_empty(), "{sh:?} 缺 parse_names");
            assert!(s.display_name.eq_ignore_ascii_case(s.shell_flag), "{sh:?} display/flag 不自洽");
            let hook = (s.generate_hook)();
            assert!(hook.contains(ENV_HOOK_BASE_PATH), "{sh:?} hook 缺 BASE_PATH 保存");
            assert!(hook.contains("sdkm env"), "{sh:?} hook 缺 env 调用");
            assert!(hook.is_ascii(), "{sh:?} hook 输出含非 ASCII（GBK 代码页下会破坏脚本）");
            assert!(!(s.base_self_heal_line)().is_empty());
            assert!(!(s.export_line)("K", "v").is_empty());
            assert!(!(s.unset_line)("K").is_empty());
        }
        // persistence：bash/zsh/fish 必须 Some 且字段满足各自 path_model；PowerShell 必须缺席
        for sh in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let p = sh.persistence().unwrap_or_else(|| panic!("{sh:?} 应提供持久化表"));
            assert_eq!(p.shell, sh);
            assert!(!(p.echo_path_cmd)().is_empty());
            match p.path_model {
                PathModel::RebuildLine => {
                    assert!(p.profile_path_line.is_some(), "{sh:?} RebuildLine 缺 profile_path_line");
                    assert!(p.parse_profile_path_line.is_some(), "{sh:?} RebuildLine 缺 parse");
                    assert!(p.add_dir_command.is_none());
                }
                PathModel::PerDirCommand => {
                    assert!(p.add_dir_command.is_some(), "{sh:?} PerDirCommand 缺 add_dir_command");
                    assert!(p.profile_path_line.is_none());
                    assert!(p.parse_profile_path_line.is_none());
                }
            }
        }
        assert!(Shell::PowerShell.persistence().is_none(), "PowerShell 不应有 unix 持久化表");
    }

    /// fish 语法 golden
    #[test]
    fn fish_syntax_golden() {
        let f = Shell::Fish.syntax();
        assert_eq!((f.export_line)("JAVA_HOME", "/x y"), "set -gx JAVA_HOME \"/x y\"");
        assert_eq!((f.unset_line)("KEY"), "set -q KEY; and set -e KEY");
        assert_eq!(
            (f.path_line)(&["a".to_string(), "b".to_string()]),
            format!("set -gx PATH \"a\" \"b\" ${}", ENV_HOOK_BASE_PATH)
        );
        // PATH 拆分为条目由 fish 的 list 语义承担；base 不引号 = list 展开
        assert!(!(f.path_line)(&[]).contains('"'), "无 bins 时 base 应不引号");
        let p = Shell::Fish.persistence().unwrap();
        let add = p.add_dir_command.unwrap()("/x y");
        assert_eq!(add, "fish_add_path --path \"/x y\"");
        assert_eq!((p.echo_path_cmd)(), "string join : $PATH");
    }

    /// bash RebuildLine 解析/生成往返
    #[test]
    fn bash_rebuild_line_roundtrip() {
        let p = Shell::Bash.persistence().unwrap();
        let parse = p.parse_profile_path_line.unwrap();
        let build = p.profile_path_line.unwrap();
        let (entries, backref) = parse("export PATH=\"/a:/b:$PATH\"");
        assert_eq!(entries, vec!["/a".to_string(), "/b".to_string()]);
        assert!(backref);
        assert_eq!(build(&entries, backref), "export PATH=\"/a:/b:$PATH\"");
        // 无 backref 行
        let (e2, b2) = parse("export PATH=\"/a:/b\"");
        assert!(!b2);
        assert_eq!(build(&e2, b2), "export PATH=\"/a:/b\"");
    }
}
