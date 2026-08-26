// ──────────────────────────────────────────────────────
// Unix 环境操作：修改 shell profile（按 $SHELL 检测到的 shell 定位）+ source 生效。
// 持久化语法全部来自 util::shell_backend::ProfilePersistence：
//   bash/zsh → RebuildLine（`export PATH="a:b:$PATH"` 单行整体重建）
//   fish     → PerDirCommand（每目录一行 `fish_add_path --path "<dir>"`）
// PowerShell 无 unix 持久化（走 Windows 注册表），detect_shell 在 unix 不出 PowerShell。
// ──────────────────────────────────────────────────────

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use util::consts::ENV_PATH;
use util::shell::{PathModel, ProfilePersistence, detect_shell};
use util::{detail, info, warning};

use crate::env::EnvOperation;

/// RebuildLine 模型（bash/zsh）的 PATH 行行首前缀——两者共用同一 `export PATH=` 形态
const REBUILD_PATH_PREFIX: &str = "export PATH=";

pub struct UnixEnvOperation {}

impl UnixEnvOperation {
    /// 当前 shell 的持久化表（unix 下 detect 结果必为 bash/zsh/fish，均有持久化）
    fn current_persistence() -> Result<&'static ProfilePersistence> {
        detect_shell()
            .persistence()
            .context("current shell has no unix profile persistence")
    }

    /// 按当前 shell 定位 profile（detect 走 util::shell，fish 用户命中 config.fish 而非 .bashrc）
    fn profile_path() -> Result<PathBuf> {
        detect_shell()
            .profile_paths()?
            .into_iter()
            .next()
            .context("cannot determine unix profile path")
    }

    /// 展开 `~` 前缀（bash/zsh/fish 的 profile 路径都接受绝对路径，直接写 HOME 展开后的值）
    fn expand_path(path: &str) -> String {
        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                return path.replacen('~', &home, 1);
            }
        }
        path.to_string()
    }

    /// 读 profile 全文；文件不存在或读失败返回空串（调用方依赖此语义）
    fn read_profile(file_path: &Path) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    /// 把行数组写回 profile（\n 连接）
    fn write_profile(file_path: &Path, lines: &[String]) -> Result<()> {
        fs::write(file_path, lines.join("\n"))?;
        Ok(())
    }

    /// 在 profile 行里找 `export PATH=` 行的索引（RebuildLine 专用）
    fn find_path_export_line(lines: &[String]) -> Option<usize> {
        lines
            .iter()
            .position(|l| l.trim().starts_with(REBUILD_PATH_PREFIX))
    }

    /// 从 profile 内容提取 PATH 值；无 export PATH 行时回退到进程 $PATH（RebuildLine 专用）
    fn get_path_from_content(content: &str) -> String {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(REBUILD_PATH_PREFIX) {
                return line
                    .trim_start_matches(REBUILD_PATH_PREFIX)
                    .replace('"', "")
                    .to_string();
            }
        }
        env::var(ENV_PATH).unwrap_or_default()
    }

    /// 设置/替换一个赋值行（已存在则替换，无则追加）。行形态走 backend（bash `export K="v"` / fish `set -gx K "v"`）
    fn append_or_replace_export(
        file_path: &Path,
        p: &'static ProfilePersistence,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let expanded_value = Self::expand_path(value);
        // 赋值行来自 ShellSyntax（export_line 是脚本语法，persistence 表只提供匹配前缀 export_prefix）
        let new_line = (p.shell.syntax().export_line)(key, &expanded_value);
        let export_prefix = (p.export_prefix)(key);
        let mut lines: Vec<String> = Self::read_profile(file_path)
            .lines()
            .map(String::from)
            .collect();

        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with(&export_prefix) {
                *line = new_line.clone();
                found = true;
                break;
            }
        }
        if !found {
            lines.push(new_line);
        }
        Self::write_profile(file_path, &lines)
    }

    /// 把 new_path 前置到 PATH（按 path_model 分发；p 由调用方传入便于测试直测两种模型）
    ///
    /// RebuildLine（bash/zsh）：解析现有 PATH 行 → 前置插入 → 整行重建（已存在则跳过并 warning）。
    /// PerDirCommand（fish）：直接追加一行 `fish_add_path --path "<dir>"`——幂等由 fish_add_path
    /// 自身保证，无需预先去重检查（含空格路径由引号参数原生支持）。
    fn add_path_entry(file_path: &Path, p: &'static ProfilePersistence, new_path: &str) -> Result<()> {
        let expanded = Self::expand_path(new_path);
        match p.path_model {
            PathModel::RebuildLine => {
                let parse = p.parse_profile_path_line.context("RebuildLine 缺 parse 语法")?;
                let build = p.profile_path_line.context("RebuildLine 缺 build 语法")?;
                let content = Self::read_profile(file_path);

                // 已存在则跳过（检查 export PATH 行值与进程 $PATH）
                if Self::get_path_from_content(&content)
                    .split(':')
                    .any(|e| e == expanded)
                {
                    warning!("path exists. sdk_path: {}", new_path);
                    return Ok(());
                }

                let mut lines: Vec<String> = content.lines().map(String::from).collect();
                if let Some(idx) = Self::find_path_export_line(&lines) {
                    // 前置插入到已有 export PATH 行；保留原 backref 状态（有 $PATH 引用则重建时附回）
                    let (entries, backref) = parse(&lines[idx]);
                    let mut merged = vec![expanded.clone()];
                    merged.extend(entries);
                    lines[idx] = build(&merged, backref);
                } else {
                    // 无 export PATH 行：新建，backref=$PATH 引用（$PATH 展开避免冲掉系统 PATH）
                    lines.push(build(std::slice::from_ref(&expanded), true));
                }
                Self::write_profile(file_path, &lines)
            }
            PathModel::PerDirCommand => {
                let add = p.add_dir_command.context("PerDirCommand 缺 add_dir_command")?;
                let line = add(&expanded);
                let mut content = Self::read_profile(file_path);
                if !content.is_empty() && !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str(&line);
                content.push('\n');
                fs::write(file_path, content)?;
                Ok(())
            }
        }
    }

    /// 起子 shell source profile，把新 PATH 写回当前进程
    ///
    /// shell 命令与「输出 PATH 的协议」来自 backend：bash/zsh `echo $PATH`（冒号分隔串）；
    /// fish 的 `$PATH` 是 list，`echo` 会每路径一行 → 必须用 `string join : $PATH` 转冒号串，
    /// 否则当前进程 PATH 会被写坏。（fish -c 本会自动读 config.fish，显式 source 冗余但无害，保留对称。）
    fn source_profile(profile_path: &Path) -> Result<()> {
        let p = Self::current_persistence()?;
        let source_cmd = format!(
            "source '{}' 2>/dev/null; {}",
            profile_path.display(),
            (p.echo_path_cmd)()
        );
        let output = Command::new(p.shell_command).args(["-c", &source_cmd]).output()?;

        if output.status.success() {
            let new_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !new_path.is_empty() {
                // 2024 edition: env::set_var 已标 unsafe（多线程 UB 风险）。
                // sdkm 单线程 CLI 场景功能上生效；彻底修复需重构为 shim 模式（不写当前进程）。
                unsafe {
                    env::set_var(ENV_PATH, new_path);
                }
            }
        }

        Ok(())
    }
}

impl EnvOperation for UnixEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<()> {
        let p = Self::current_persistence()?;
        let profile_path = Self::profile_path()?;
        for (env_key, env_val) in envs {
            Self::append_or_replace_export(&profile_path, p, env_key, env_val)?;
            info!("success set env key:`{}` value:`{}` !", env_key, env_val);
        }
        Self::source_profile(&profile_path)?;
        Ok(())
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()> {
        let p = Self::current_persistence()?;
        let profile_path = Self::profile_path()?;
        Self::add_path_entry(&profile_path, p, sdk_path)?;
        info!("success add `{}` to path!", sdk_path);
        Self::source_profile(&profile_path)?;
        Ok(())
    }

    fn get_path(&self) -> Result<String> {
        Ok(env::var(ENV_PATH).unwrap_or_default())
    }

    fn remove_sdk_path(&self, target: &str) -> Result<()> {
        let profile_path = Self::profile_path()?;
        let p = Self::current_persistence()?;
        let expanded_target = Self::expand_path(target);

        let content = Self::read_profile(&profile_path);
        if content.is_empty() {
            return Ok(()); // 文件不存在，无需移除
        }

        match p.path_model {
            PathModel::RebuildLine => {
                let parse = p.parse_profile_path_line.context("RebuildLine 缺 parse 语法")?;
                let build = p.profile_path_line.context("RebuildLine 缺 build 语法")?;
                let mut lines: Vec<String> = content.lines().map(String::from).collect();
                if let Some(idx) = Self::find_path_export_line(&lines) {
                    let (entries, backref) = parse(&lines[idx]);
                    let filtered: Vec<String> = entries
                        .into_iter()
                        .filter(|e| e != &expanded_target.as_str() && e != target)
                        .collect();
                    // 保留 :$PATH 引用（backref），否则 export PATH="<paths>" 会冲掉系统 PATH
                    if filtered.is_empty() {
                        lines.remove(idx);
                    } else {
                        lines[idx] = build(&filtered, backref);
                    }
                }
                Self::write_profile(&profile_path, &lines)?;
            }
            PathModel::PerDirCommand => {
                // fish：精确删掉匹配 add_dir_command(target) 的行（与 add 时写入的字符串对齐）
                let add = p.add_dir_command.context("PerDirCommand 缺 add_dir_command")?;
                let expect = add(&expanded_target);
                let lines: Vec<String> = content
                    .lines()
                    .filter(|l| l.trim() != expect)
                    .map(String::from)
                    .collect();
                Self::write_profile(&profile_path, &lines)?;
            }
        }

        Self::source_profile(&profile_path)?;
        detail!("removed `{target}` from PATH");
        Ok(())
    }

    fn get_env_value(&self, key: &str) -> Result<Option<String>> {
        let p = Self::current_persistence()?;
        let profile_path = Self::profile_path()?;
        let export_prefix = (p.export_prefix)(key);
        for line in Self::read_profile(&profile_path).lines() {
            let line = line.trim();
            if line.starts_with(&export_prefix) {
                let value = line.trim_start_matches(&export_prefix).trim_matches('"');
                return Ok(Some(value.to_string()));
            }
        }
        // profile 里没有，回退到进程环境变量
        Ok(env::var(key).ok())
    }

    fn unset_sdk_env(&self, key: &str) -> Result<()> {
        let p = Self::current_persistence()?;
        let profile_path = Self::profile_path()?;
        let content = Self::read_profile(&profile_path);
        if content.is_empty() {
            return Ok(()); // 文件不存在，无需移除
        }

        let export_prefix = (p.export_prefix)(key);
        let lines: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().starts_with(&export_prefix))
            .map(String::from)
            .collect();

        Self::write_profile(&profile_path, &lines)?;
        Self::source_profile(&profile_path)?;
        detail!("removed env `{key}` from shell profile");
        Ok(())
    }

    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<()> {
        let p = Self::current_persistence()?;
        let profile_path = Self::profile_path()?;
        for (env_key, old_val) in old_envs {
            if let Some(val) = old_val {
                Self::append_or_replace_export(&profile_path, p, env_key, val)?;
                info!("restored env `{env_key}` to `{val}`");
            } else {
                self.unset_sdk_env(env_key)?;
            }
        }
        Self::source_profile(&profile_path)?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use util::shell::Shell;

    /// 临时唯一 profile 路径（bash 语义；测试不改真实 profile）
    fn temp_profile() -> PathBuf {
        let dir = env::temp_dir();
        let name = format!(
            "sdkm_unix_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        );
        dir.join(name)
    }

    /// bash RebuildLine：export 写入 → 读回 → 替换 → 取消
    #[test]
    fn bash_export_write_replace_read() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_profile();

        // 新建写入
        UnixEnvOperation::append_or_replace_export(&path, p, "JAVA_HOME", "/sdk/java").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("export JAVA_HOME=\"/sdk/java\""));

        // 覆盖替换
        UnixEnvOperation::append_or_replace_export(&path, p, "JAVA_HOME", "/sdk/java2").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().filter(|l| l.contains("JAVA_HOME")).count(), 1);
        assert!(content.contains("export JAVA_HOME=\"/sdk/java2\""));

        let _ = fs::remove_file(&path);
    }

    /// bash RebuildLine：add_path_entry 新建 PATH 行（backref=$PATH）→ 解析回读
    #[test]
    fn bash_rebuild_line_add_path_entry() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_profile();
        UnixEnvOperation::add_path_entry(&path, p, "/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "export PATH=\"/sdk/bin:$PATH\"\n");
        let _ = fs::remove_file(&path);
    }

    /// fish PerDirCommand：add_path_entry 直接追加 fish_add_path 行（真正走 add_path_entry 的
    /// fish 分支，幂等由 fish_add_path 承担，无需去重）
    #[test]
    fn fish_per_dir_add_path_entry() {
        let p = Shell::Fish.persistence().unwrap();
        let path = temp_profile();
        UnixEnvOperation::add_path_entry(&path, p, "/opt/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "fish_add_path --path \"/opt/sdk/bin\"\n");
        let _ = fs::remove_file(&path);
    }
}