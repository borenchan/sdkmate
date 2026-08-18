// ──────────────────────────────────────────────────────
// Unix 环境操作：修改 shell profile（~/.bashrc 或 ~/.zshrc）+ source 生效
// ──────────────────────────────────────────────────────

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use util::consts::ENV_PATH;
use util::{detail, info, warning};

use crate::env::EnvOperation;

// ── 魔法值集中管理 ──
const PATH_SEPARATOR: &str = ":"; // Unix PATH 分隔符（Windows 是 ;）
const PROFILE_BASHRC: &str = ".bashrc";
const PROFILE_ZSHRC: &str = ".zshrc";
const EXPORT_PREFIX: &str = "export ";
const PATH_EXPORT_PREFIX: &str = "export PATH=";
/// 新建 export PATH 行时引用 $PATH，避免 source 后冲掉系统 PATH
const PATH_BACKREF: &str = "$PATH";

pub struct UnixEnvOperation {}

impl UnixEnvOperation {
    /// 按 $SHELL 选 profile：zsh → ~/.zshrc，其余 → ~/.bashrc（统一走 util::shell，避免重复判定）
    fn get_shell_profile_path() -> Result<PathBuf> {
        util::shell::unix_profile_path()
    }

    fn expand_path(path: &str) -> String {
        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                return path.replacen('~', &home, 1);
            }
        }
        path.to_string()
    }

    /// 读 profile 全文；文件不存在或读失败返回空串（调用方依赖此语义）
    fn read_profile(file_path: &PathBuf) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    /// 把行数组写回 profile（\n 连接）
    fn write_profile(file_path: &PathBuf, lines: &[String]) -> Result<()> {
        fs::write(file_path, lines.join("\n"))?;
        Ok(())
    }

    /// 在 profile 行里找 `export PATH=` 行的索引
    fn find_path_export_line(lines: &[String]) -> Option<usize> {
        lines.iter().position(|l| l.trim().starts_with(PATH_EXPORT_PREFIX))
    }

    /// 从 profile 内容提取 PATH 值；无 export PATH 行时回退到进程 $PATH
    fn get_path_from_content(content: &str) -> String {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(PATH_EXPORT_PREFIX) {
                return line.trim_start_matches(PATH_EXPORT_PREFIX).replace('"', "").to_string();
            }
        }
        env::var(ENV_PATH).unwrap_or_default()
    }

    /// 设置/替换一个 `export <key>="<value>"` 行（已存在则替换，无则追加）
    fn append_or_replace_export(file_path: &PathBuf, key: &str, value: &str) -> Result<()> {
        let expanded_value = Self::expand_path(value);
        let new_line = format!("{}{}=\"{}\"", EXPORT_PREFIX, key, expanded_value);
        let mut lines: Vec<String> = Self::read_profile(file_path).lines().map(String::from).collect();

        let export_pattern = format!("{}{}=", EXPORT_PREFIX, key);
        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with(&export_pattern) {
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

    /// 把 new_path 前置到 PATH（已存在则跳过并 warning）
    fn add_path_entry(file_path: &PathBuf, new_path: &str) -> Result<()> {
        let expanded_path = Self::expand_path(new_path);
        let content = Self::read_profile(file_path);

        // 已存在则跳过（检查 PATH 各条目，覆盖 export PATH 行值与进程 $PATH）
        if Self::get_path_from_content(&content)
            .split(PATH_SEPARATOR)
            .any(|p| p == expanded_path)
        {
            warning!("path exists. sdk_path: {}", new_path);
            return Ok(());
        }

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if let Some(idx) = Self::find_path_export_line(&lines) {
            // 前置插入到已有 export PATH 行；整体引号包裹值，replace 去引号解析兼容历史部分引号格式
            let current_value = lines[idx].trim_start_matches(PATH_EXPORT_PREFIX).replace('"', "");
            lines[idx] = format!("{}\"{}{}{}\"", PATH_EXPORT_PREFIX, expanded_path, PATH_SEPARATOR, current_value);
        } else {
            // 无 export PATH 行：新建，整体引号包裹 <dir>:$PATH（$PATH 在双引号内会展开，避免冲掉系统 PATH）
            lines.push(format!(
                "{}\"{}{}{}\"",
                PATH_EXPORT_PREFIX, expanded_path, PATH_SEPARATOR, PATH_BACKREF
            ));
        }
        Self::write_profile(file_path, &lines)
    }

    /// 起子 shell source profile，把新 PATH 写回当前进程
    fn source_profile(profile_path: &PathBuf) -> Result<()> {
        let shell = env::var("SHELL").unwrap_or_default();
        let is_zsh = shell.contains("zsh");
        let profile_name = profile_path.file_name().and_then(|n| n.to_str()).unwrap_or(if is_zsh {
            PROFILE_ZSHRC
        } else {
            PROFILE_BASHRC
        });

        let source_cmd = format!("source '{}' 2>/dev/null; echo $PATH", profile_path.display());
        let output = if is_zsh {
            Command::new("zsh").args(["-c", &source_cmd]).output()?
        } else {
            Command::new("bash").args(["-c", &source_cmd]).output()?
        };

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
        let profile_path = Self::get_shell_profile_path()?;
        for (env_key, env_val) in envs {
            Self::append_or_replace_export(&profile_path, env_key, env_val)?;
            info!("success set env key:`{}` value:`{}` !", env_key, env_val);
        }
        Self::source_profile(&profile_path)?;
        Ok(())
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;
        Self::add_path_entry(&profile_path, sdk_path)?;
        info!("success add `{}` to path!", sdk_path);
        Self::source_profile(&profile_path)?;
        Ok(())
    }

    fn get_path(&self) -> Result<String> {
        Ok(env::var(ENV_PATH).unwrap_or_default())
    }

    fn remove_sdk_path(&self, target: &str) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;
        let expanded_target = Self::expand_path(target);

        let content = Self::read_profile(&profile_path);
        if content.is_empty() {
            return Ok(()); // 文件不存在，无需移除
        }

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if let Some(idx) = Self::find_path_export_line(&lines) {
            let current_value = lines[idx].trim_start_matches(PATH_EXPORT_PREFIX).replace('"', "");
            let paths: Vec<String> = current_value
                .split(PATH_SEPARATOR)
                .filter(|&p| p != expanded_target.as_str() && p != target)
                .map(String::from)
                .collect();
            // 保留 :$PATH 引用（整体引号包裹），否则 export PATH="<paths>" 会冲掉系统 PATH
            if paths.is_empty() {
                lines.remove(idx);
            } else {
                lines[idx] = format!(
                    "{}\"{}{}{}\"",
                    PATH_EXPORT_PREFIX,
                    paths.join(PATH_SEPARATOR),
                    PATH_SEPARATOR,
                    PATH_BACKREF
                );
            }
        }

        Self::write_profile(&profile_path, &lines)?;
        Self::source_profile(&profile_path)?;
        detail!("removed `{target}` from PATH");
        Ok(())
    }

    fn get_env_value(&self, key: &str) -> Result<Option<String>> {
        let profile_path = Self::get_shell_profile_path()?;
        let export_pattern = format!("{}{}=", EXPORT_PREFIX, key);
        for line in Self::read_profile(&profile_path).lines() {
            let line = line.trim();
            if line.starts_with(&export_pattern) {
                let value = line.trim_start_matches(&export_pattern).trim_matches('"');
                return Ok(Some(value.to_string()));
            }
        }
        // profile 里没有，回退到进程环境变量
        Ok(env::var(key).ok())
    }

    fn unset_sdk_env(&self, key: &str) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;
        let content = Self::read_profile(&profile_path);
        if content.is_empty() {
            return Ok(()); // 文件不存在，无需移除
        }

        let export_pattern = format!("{}{}=", EXPORT_PREFIX, key);
        let lines: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().starts_with(&export_pattern))
            .map(String::from)
            .collect();

        Self::write_profile(&profile_path, &lines)?;
        Self::source_profile(&profile_path)?;
        detail!("removed env `{key}` from shell profile");
        Ok(())
    }

    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;
        for (env_key, old_val) in old_envs {
            if let Some(val) = old_val {
                Self::append_or_replace_export(&profile_path, env_key, val)?;
                info!("restored env `{env_key}` to `{val}`");
            } else {
                self.unset_sdk_env(env_key)?;
            }
        }
        Self::source_profile(&profile_path)?;
        Ok(())
    }
}
