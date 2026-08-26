//! `sdkm env` 的输出生成器（高频，带缓存）：输出当前目录应 eval 的环境变量设置脚本。
//!
//! 三层优先级解析（对每个注册 SDK）：会话层 `SDKM_ACTIVE_<SDK>`（最高）→ 项目层
//! `.sdkm.toml` pins → 全局层（不加 prepend，PATH 依赖全局 symlink 已在 base）。
//!
//! 生成脚本结构（幂等重建，无跨调用状态）：
//! 1. PATH 行：`export PATH="<项目 bins 字母序拼接>:$_SDKM_BASE_PATH"`（无项目 bins
//!    时 = base 本身——离开项目目录即还原）
//! 2. env vars：对本次选中的 extra_vars keys 输出 export；对全局 config 所有 SDK 的
//!    extra_vars keys 并集中本次未选中的输出 unset（known 集合幂等重建，离开项目还原）
//!
//! **stdout 纯净性**：本模块输出会被 shell `eval`，全程禁用 info!/warning!/success!
//! 等 stdout 宏；诊断一律走 stderr（error! 宏或 eprintln!）。

use crate::config::SdkConfig;
use crate::hook_cache::{CACHE_SCHEMA_VERSION, HookCache, HookEntry, file_mtime_nanos};
use crate::manager::SdkManager;
use crate::project_config::find_project_config;
use crate::version::fuzzy_match_version_core;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use util::config_helper::PLACEHOLDER_SDK_DIR;
use util::consts::SDKM_SESSION_ENV_PREFIX;
use util::error;
use util::sdk::Sdk;
use util::shell::Shell;

/// 单个选中 SDK 的待注入环境（bin 路径 + 渲染后的 env vars）
struct SelectedSdk {
    bins: Vec<String>,
    env_vars: Vec<(String, String)>,
}

impl SdkManager {
    /// 生成 env 脚本（缓存包装：命中直接吐，miss 实时解析并回写）
    ///
    /// 缓存 IO 任何失败都静默降级到实时解析（缓存纯优化）。
    pub fn generate_env_script_cached(&self, shell: Shell, pwd: &Path) -> String {
        let mut cache = HookCache::load();
        cache.prune();
        if let Some(entry) = cache.resolve(pwd, shell as u8) {
            return entry.env_script.clone();
        }
        let (script, config_path, mtime) = self.generate_env_script_inner(shell, pwd);
        if let (Some(path), Some(m)) = (config_path, mtime) {
            cache.put(
                pwd,
                HookEntry {
                    config_path: path.to_string_lossy().into_owned(),
                    mtime_nanos: m,
                    env_script: script.clone(),
                    schema_version: CACHE_SCHEMA_VERSION,
                    // 记录 shell：同一 PWD 跨 shell 调用时命中校验不符当 miss，防脚本串扰
                    shell: shell as u8,
                },
            );
            cache.save();
        }
        script
    }

    /// 实时解析生成 env 脚本（无缓存）。返 (脚本, 命中的配置文件路径, 其 mtime)
    ///
    /// 无项目配置时 config_path/mtime 为 None（该 PWD 不缓存——「无配置」状态
    /// 可能因用户创建配置而改变，无法用 mtime 锚定，每次实时解析）。
    fn generate_env_script_inner(&self, shell: Shell, _pwd: &Path) -> (String, Option<PathBuf>, Option<u128>) {
        // 三层解析：会话层（SDKM_ACTIVE_*）> 项目层（.sdkm.toml）> 全局（跳过）
        let mut selected: Vec<(String, SelectedSdk)> = Vec::new();

        // 会话层最高优先：遍历所有注册 SDK，SDKM_ACTIVE_<UPPER> 已设的用会话版本
        for sdk_conf in &self.config.sdks {
            if let Some(version) = session_env_value(&sdk_conf.name) {
                match self.select_sdk_envs(&sdk_conf.name, &version) {
                    Some(env_data) => selected.push((sdk_conf.name.clone(), env_data)),
                    None => continue, // 未装/解析失败：error 已走 stderr，跳过不阻断
                }
            }
        }

        // 项目层：.sdkm.toml pins（会话层已覆盖的 SDK 跳过）
        let project = find_project_config().ok().flatten();
        if let Some((config_path, project_cfg)) = project {
            for (name, version_input) in &project_cfg.pins {
                if session_env_value(name).is_some() {
                    continue; // 会话层优先于项目层
                }
                match self.select_sdk_envs(name, version_input) {
                    Some(env_data) => selected.push((name.clone(), env_data)),
                    None => continue, // 未装/解析失败：error 已走 stderr，跳过不阻断
                }
            }
            let script = render_env_script(shell, &selected, &known_env_keys(&self.config.sdks));
            let mtime = file_mtime_nanos(&config_path);
            (script, Some(config_path), mtime)
        } else {
            // 无项目配置：仅会话层选中的 + PATH 重建 + 全量 unset（幂等还原语义）
            let script = render_env_script(shell, &selected, &known_env_keys(&self.config.sdks));
            (script, None, None)
        }
    }

    /// 解析单个 SDK 的项目级环境注入（bin 路径 + extra_vars 渲染）
    ///
    /// 未安装/未注册/fuzzy 失败 → None（诊断走 stderr，跳过不阻断 shell）
    fn select_sdk_envs(&self, name: &str, version_input: &str) -> Option<SelectedSdk> {
        let sdk = match Sdk::from_str(name) {
            Ok(s) => s,
            Err(_) => {
                error!("project config: unknown sdk `{}`, skipped", name);
                return None;
            }
        };
        let sdk_conf = match self.config.find_sdk(&sdk) {
            Some(c) => c,
            None => {
                error!("project config: unregistered sdk `{}`, skipped", name);
                return None;
            }
        };

        // 模糊匹配本地已装版本（与 switch 同源）
        let versions = self.list_local_sdk_versions(&sdk).ok()?;
        if versions.is_empty() {
            error!(
                "project config: `{}` has no installed versions, `{}` falls back to global",
                name, version_input
            );
            return None;
        }
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = match fuzzy_match_version_core(&version_strings, version_input) {
            Ok(m) => m,
            Err(e) => {
                error!("project config: `{}` {}", name, e);
                return None;
            }
        };
        let target = matched.full_version;
        let item = versions.into_iter().find(|v| v.sdk_version == target)?;

        // 拼项目层 PATH 条目：store 真实版本目录 + bin_dir（绕过全局 symlink）
        let mut bins = Vec::new();
        let main_bin = item.sdk_dir.join(sdk_conf.bin_dir.as_deref().unwrap_or(""));
        bins.push(main_bin.to_string_lossy().into_owned());
        // extra_paths 照搬 switch 的处理（如 Python Windows 的 Scripts）
        for extra in &sdk_conf.extra_paths {
            let extra_dir = item.sdk_dir.join(extra);
            if extra_dir.exists() {
                bins.push(extra_dir.to_string_lossy().into_owned());
            }
        }

        // extra_vars 渲染：{sdk_dir} = store 真实版本目录（与 switch 的 symlink 语义解耦）
        let mut env_vars = Vec::new();
        let mut dynamic: HashMap<&str, &str> = HashMap::new();
        let sdk_dir_str = item.sdk_dir.to_string_lossy();
        dynamic.insert(PLACEHOLDER_SDK_DIR, sdk_dir_str.as_ref());
        let rendered = sdk_conf.get_actual_extra_vars(&dynamic).ok()?;
        for (k, v) in rendered {
            env_vars.push((k, v));
        }

        Some(SelectedSdk { bins, env_vars })
    }
}

/// 会话层载体读取：`SDKM_ACTIVE_<UPPER>` 已设且非空 → 返回其值（该 SDK 用会话版本）
fn session_env_value(sdk_name: &str) -> Option<String> {
    let var = format!("{}{}", SDKM_SESSION_ENV_PREFIX, sanitize_env_suffix(sdk_name));
    env::var(&var).ok().filter(|v| !v.is_empty())
}

/// SDK 名转合法 env var 后缀：大写 + 非字母数字替 `_`（custom SDK 可含 `-`）
fn sanitize_env_suffix(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// 全局 config 所有 SDK 的 extra_vars keys 并集（known 集合，幂等 unset 用）
fn known_env_keys(sdks: &[SdkConfig]) -> BTreeSet<String> {
    sdks.iter().flat_map(|s| s.extra_vars.keys().cloned()).collect()
}

/// 渲染最终 env 脚本：PATH 重建行 + unset（known - cur）+ export（cur）
fn render_env_script(shell: Shell, selected: &[(String, SelectedSdk)], known: &BTreeSet<String>) -> String {
    let cur: BTreeSet<&str> = selected
        .iter()
        .flat_map(|(_, s)| s.env_vars.iter().map(|(k, _)| k.as_str()))
        .collect();

    // 项目 bins：按 BTreeMap 迭代序（SDK 名字母序）拼接——确定性的 PATH 顺序
    let mut path_entries: Vec<String> = Vec::new();
    for (_, s) in selected {
        path_entries.extend(s.bins.iter().cloned());
    }

    // 渲染语法统一走 ShellSyntax fn（bash/zsh/PS/fish 各自实现；顺序固定：base 自愈 → PATH → unset → export）
    let b = shell.syntax();
    let mut lines = Vec::new();
    // base 兜底自愈：hook 未注入时（手动 eval/source/IEX）先锚定 base = 当前 PATH，防 PATH 被清空
    lines.push((b.base_self_heal_line)());
    // PATH 重建行（bins 字母序拼接，引用 _SDKM_BASE_PATH；无 bins = base 本身，离开项目还原）
    lines.push((b.path_line)(&path_entries));
    // unset（known ∖ cur）：对全局 config 所有 extra_vars keys 并集中本次未选中的输出取消，幂等还原
    for key in known.iter().filter(|k| !cur.contains(k.as_str())) {
        lines.push((b.unset_line)(key));
    }
    // export（cur）：对本次选中的 extra_vars 输出赋值
    for (_, s) in selected {
        for (k, v) in &s.env_vars {
            lines.push((b.export_line)(k, v));
        }
    }

    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use util::consts::ENV_HOOK_BASE_PATH;

    /// 构造一个选中 SDK（2 个 bins + 1 个 extra_var）与 known 集合（含一个未选中的 EXTRA）
    fn sample_selected() -> Vec<(String, SelectedSdk)> {
        vec![(
            "java".to_string(),
            SelectedSdk {
                bins: vec!["bin/a".to_string(), "bin/b".to_string()],
                env_vars: vec![("JAVA_HOME".to_string(), "/sdk/java".to_string())],
            },
        )]
    }
    fn sample_known() -> BTreeSet<String> {
        ["JAVA_HOME".to_string(), "EXTRA".to_string()].into_iter().collect()
    }

    /// render_env_script 是纯函数，四 shell 各自语法 golden（顺序固定：base 自愈 → PATH → unset → export）
    #[test]
    fn render_env_script_fish_golden() {
        let e = ENV_HOOK_BASE_PATH;
        let script = render_env_script(Shell::Fish, &sample_selected(), &sample_known());
        assert_eq!(
            script,
            format!(
                "if not set -q {e}\n    set -gx {e} $PATH\nend\n\
                 set -gx PATH \"bin/a\" \"bin/b\" ${e}\n\
                 set -q EXTRA; and set -e EXTRA\n\
                 set -gx JAVA_HOME \"/sdk/java\"\n"
            )
        );
    }

    #[test]
    fn render_env_script_bash_golden() {
        let e = ENV_HOOK_BASE_PATH;
        let script = render_env_script(Shell::Bash, &sample_selected(), &sample_known());
        assert_eq!(
            script,
            format!(
                "[ -z \"${{{e}:-}}\" ] && export {e}=\"$PATH\"\n\
                 export PATH=\"bin/a:bin/b:${e}\"\n\
                 unset EXTRA\n\
                 export JAVA_HOME=\"/sdk/java\"\n"
            )
        );
        // zsh 与 bash 语法相同（同 backend fn）
        assert_eq!(script, render_env_script(Shell::Zsh, &sample_selected(), &sample_known()));
    }

    #[test]
    fn render_env_script_powershell_golden() {
        let e = ENV_HOOK_BASE_PATH;
        let script = render_env_script(Shell::PowerShell, &sample_selected(), &sample_known());
        assert_eq!(
            script,
            format!(
                "if (-not $env:{e}) {{ $env:{e} = $env:PATH }}\n\
                 $env:PATH = \"bin/a;bin/b;\" + $env:{e}\n\
                 Remove-Item Env:EXTRA -ErrorAction SilentlyContinue\n\
                 $env:JAVA_HOME = \"/sdk/java\"\n"
            )
        );
    }
}
