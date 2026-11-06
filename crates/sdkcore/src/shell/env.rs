//! `sdkm env` 的输出生成器（高频，带缓存）：输出当前目录应 eval 的环境变量设置脚本。
//!
//! 三层优先级解析（对每个注册 SDK）：会话层 `SDKM_ACTIVE_<SDK>`（最高）→ 项目层
//! `.sdkm.toml` pins → 全局层（config 里 `current_version` 已设的 SDK，symlink bin
//! 路径——switch 后按回车 hook 即重建 PATH 生效，无需重启终端）。
//!
//! 生成脚本结构（贵的三层解析结果进缓存；PATH 行依赖运行期 shell 状态实时拼装）：
//! 1. base 自愈行：锚定 `_SDKM_BASE_PATH`（hook 未注入时手动 eval 的兜底）
//! 2. PATH 行：外科手术重组（[`rebuild_path_entries`]）——读父 shell 实时 `$PATH`、
//!    `_SDKM_INJECTED_PATHS`（上次注入记录）与 `_SDKM_BASE_PATH`（启动快照），剥旧注新
//!    并按**分层语义**归位：会话/项目条目顶最前（压过 venv 等外部前插），全局条目贴
//!    启动快照边界（外部前插保持在它前面）；其余条目原样保留，输出保首现去重
//! 3. INJECTED 记录行：更新 `_SDKM_INJECTED_PATHS` = 本次注入条目（两段拍平）
//! 4. env vars：对本次选中的 extra_vars keys 输出 export；对全局 config 所有 SDK 的
//!    extra_vars keys 并集中本次未选中的输出 unset（known 集合幂等重建，离开项目还原）
//!
//! **stdout 纯净性**：本模块输出会被 shell `eval`，全程禁用 info!/warning!/success!
//! 等 stdout 输出；诊断一律走 stderr（sink.error —— TerminalSink 渲染 Error 事件到 stderr）。

use crate::config::SdkConfig;
use crate::hook_cache::{
    CACHE_SCHEMA_VERSION, HookCache, HookEntry, current_session_fingerprint, file_mtime_nanos, global_fingerprint,
};
use crate::manager::SdkManager;
use crate::project_config::find_project_config;
use crate::version::fuzzy_match_version_core;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use util::config_helper::PLACEHOLDER_SDK_DIR;
use util::consts::{ENV_HOOK_BASE_PATH, ENV_HOOK_INJECTED_PATHS, PATH_LIST_SEPARATOR, SDKM_SESSION_ENV_PREFIX};
use util::sdk::Sdk;
use util::shell::Shell;

/// 单个选中 SDK 的待注入环境（bin 路径 + 渲染后的 env vars）
struct SelectedSdk {
    bins: Vec<String>,
    env_vars: Vec<(String, String)>,
}

/// env 脚本的可缓存部分：分层注入条目 + 静态 env 行（unset/export）
///
/// 分层决定重排位置：top（会话+项目 = 显式版本声明）顶最前压过外部前插；
/// global（全局底座）贴启动 PATH 快照边界、外部前插（venv 等）保持在它前面。
/// PATH 行依赖父 shell 运行期状态（实时 `$PATH` / 上次注入记录 / base），不进缓存。
struct EnvParts {
    top_entries: Vec<String>,
    global_entries: Vec<String>,
    env_lines: String,
}

impl SdkManager {
    /// 生成 env 脚本（缓存包装：命中取解析结果，miss 实时解析并回写；PATH 行两条路径都实时拼装）
    ///
    /// 缓存 IO 任何失败都静默降级到实时解析（缓存纯优化）。
    pub fn generate_env_script_cached(&self, shell: Shell, pwd: &Path) -> String {
        let mut cache = HookCache::load();
        cache.prune();
        // 运行期 shell 状态（父 shell 导出、子进程继承）：当前 PATH + 上次注入记录 + 启动快照
        let cur_path = env::var("PATH").ok();
        let injected = env::var(ENV_HOOK_INJECTED_PATHS).ok();
        let base_path = env::var(ENV_HOOK_BASE_PATH).ok();
        // 全局指纹本进程只算一次（resolve 判据 + put 采样共用；读 config.toml 是热路径唯一新增 IO）
        let gf = global_fingerprint();
        if let Some(entry) = cache.resolve(pwd, shell as u8, &gf) {
            return assemble_env_script(
                shell,
                cur_path.as_deref(),
                injected.as_deref(),
                base_path.as_deref(),
                &entry.top_entries,
                &entry.global_entries,
                &entry.env_lines,
            );
        }
        let (parts, config_path, mtime) = self.generate_env_script_inner(shell, None);
        if let (Some(path), Some(m)) = (config_path, mtime) {
            cache.put(
                pwd,
                HookEntry {
                    config_path: path.to_string_lossy().into_owned(),
                    mtime_nanos: m,
                    top_entries: parts.top_entries.clone(),
                    global_entries: parts.global_entries.clone(),
                    env_lines: parts.env_lines.clone(),
                    schema_version: CACHE_SCHEMA_VERSION,
                    // 记录 shell：同一 PWD 跨 shell 调用时命中校验不符当 miss，防脚本串扰
                    shell: shell as u8,
                    // 记录会话指纹：use --shell 改 SDKM_ACTIVE_* 后命中校验不符当 miss，
                    // 防缓存吐旧脚本压过会话层
                    session_fingerprint: current_session_fingerprint(),
                    // 记录全局指纹：switch/uninstall 改 current_version 后命中校验不符当 miss，
                    // 防缓存吐旧脚本丢全局层 bins
                    global_fingerprint: gf,
                },
            );
            cache.save();
        }
        assemble_env_script(
            shell,
            cur_path.as_deref(),
            injected.as_deref(),
            base_path.as_deref(),
            &parts.top_entries,
            &parts.global_entries,
            &parts.env_lines,
        )
    }

    /// 实时解析生成 env 脚本可缓存部分。返 (解析结果, 命中的配置文件路径, 其 mtime)
    ///
    /// 无项目配置时 config_path/mtime 为 None（该 PWD 不缓存——「无配置」状态
    /// 可能因用户创建配置而改变，无法用 mtime 锚定，每次实时解析）。
    /// 注：PWD 由缓存层 `generate_env_script_cached` 持有做 cache key，本函数用
    /// `find_project_config()`（内部 current_dir）解析——同进程下两者一致。
    /// `session_override`：`use --shell` 模拟「会话变量已设」视角（进程内尚未 export）。
    fn generate_env_script_inner(
        &self,
        shell: Shell,
        session_override: Option<(&str, &str)>,
    ) -> (EnvParts, Option<PathBuf>, Option<u128>) {
        let mut selected: Vec<(String, SelectedSdk)> = Vec::new();

        // 会话层最高优先：遍历所有注册 SDK，SDKM_ACTIVE_<UPPER> 已设的用会话版本
        for sdk_conf in &self.config.sdks {
            if let Some(version) = session_env_value(&sdk_conf.name, session_override) {
                match self.select_sdk_envs(&sdk_conf.name, &version) {
                    Some(env_data) => selected.push((sdk_conf.name.clone(), env_data)),
                    None => continue, // 未装/解析失败：error 已走 stderr，跳过不阻断
                }
            }
        }

        // 项目层：.sdkm.toml pins（会话层已覆盖的 SDK 跳过）
        let project = find_project_config().ok().flatten();
        let mut pinned_names: BTreeSet<String> = selected.iter().map(|(n, _)| n.clone()).collect();
        if let Some((config_path, project_cfg)) = project {
            for (name, version_input) in &project_cfg.pins {
                if session_env_value(name, session_override).is_some() || !pinned_names.insert(name.clone()) {
                    continue; // 会话层优先于项目层；同 SDK 重复 pin 只取第一个
                }
                match self.select_sdk_envs(name, version_input) {
                    Some(env_data) => selected.push((name.clone(), env_data)),
                    None => continue, // 未装/解析失败：error 已走 stderr，跳过不阻断
                }
            }
            // 全局层（兜底，最后拼接）：config 里 current_version 已设的 SDK → symlink bin +
            // extra_vars（{sdk_dir} 渲染成 symlink 目录，与 switch 持久化值一致）。让 hook 动态
            // 反映全局 active 状态：switch 后按回车即生效，无需重启终端。会话/项目已覆盖的跳过。
            let global = self.global_active_sdks(&pinned_names);
            let global_start = selected.len();
            selected.extend(global);

            let parts = render_env_parts(shell, &selected, global_start, &known_env_keys(&self.config.sdks));
            let mtime = file_mtime_nanos(&config_path);
            (parts, Some(config_path), mtime)
        } else {
            // 无项目配置：会话层 + 全局层 + 全量 unset（幂等还原语义）
            let pinned: BTreeSet<String> = selected.iter().map(|(n, _)| n.clone()).collect();
            let global = self.global_active_sdks(&pinned);
            let global_start = selected.len();
            selected.extend(global);
            let parts = render_env_parts(shell, &selected, global_start, &known_env_keys(&self.config.sdks));
            (parts, None, None)
        }
    }

    /// `sdkm use --shell` 的 eval 脚本：SDKM_ACTIVE 赋值行 + 即时 PATH 重建
    ///
    /// 会话层条目在 top 段，天然顶到最前（用户最新意图优先于外部前插）。
    pub fn generate_use_script(&self, shell: Shell, sdk_name: &str, version: &str) -> String {
        let var = format!("{}{}", SDKM_SESSION_ENV_PREFIX, sanitize_env_suffix(sdk_name));
        let mut script = format!("{}\n", (shell.syntax().export_line)(&var, version));
        let (parts, _, _) = self.generate_env_script_inner(shell, Some((sdk_name, version)));
        let cur_path = env::var("PATH").ok();
        let injected = env::var(ENV_HOOK_INJECTED_PATHS).ok();
        let base_path = env::var(ENV_HOOK_BASE_PATH).ok();
        script.push_str(&assemble_env_script(
            shell,
            cur_path.as_deref(),
            injected.as_deref(),
            base_path.as_deref(),
            &parts.top_entries,
            &parts.global_entries,
            &parts.env_lines,
        ));
        script
    }

    /// 全局层选中 SDK：config 里 `current_version` 已设且未被会话/项目层覆盖的 SDK
    ///
    /// 从 config 权威状态确定性推导（不解析 profile/live PATH）：switch/uninstall 维护
    /// current_version，与 switch 的 PATH 持久化条目一致（`<symlink_dir>/<sdk>[/<bin_dir>]` +
    /// `{sdk_dir}` 渲染成 symlink 目录），故 hook 重建的环境覆盖持久化语义。extra_vars 的渲染
    /// 让 java 全局 active 时 JAVA_HOME 也不再被 known-unset 冲掉。
    /// symlink_dir 解析失败 → 空表（无全局层，PATH 靠 base 兜底）。
    fn global_active_sdks(&self, covered: &BTreeSet<String>) -> Vec<(String, SelectedSdk)> {
        let symlink_root = match self.config.resolved_symlink_dir() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        let mut result = Vec::new();
        for s in self.config.sdks.iter() {
            if s.current_version.is_none() || covered.contains(&s.name) {
                continue; // 无 current_version 非全局 active；会话/项目层优先于全局层
            }
            // bin 路径：symlink 目录 + bin_dir（bin_dir None = 二进制在 symlink 根目录）
            let mut bins = vec![Path::new(&symlink_root).join(&s.name).join(s.bin_dir.as_deref().unwrap_or(""))];
            for extra in &s.extra_paths {
                let extra_dir = Path::new(&symlink_root).join(&s.name).join(extra);
                if extra_dir.exists() {
                    bins.push(extra_dir);
                }
            }
            // extra_vars 渲染：{sdk_dir} = symlink 目录（与 switch 的持久化语义一致）
            let sdk_dir_str = Path::new(&symlink_root).join(&s.name).to_string_lossy().into_owned();
            let mut dynamic = HashMap::new();
            dynamic.insert(PLACEHOLDER_SDK_DIR, sdk_dir_str.as_str());
            let Ok(rendered) = s.get_actual_extra_vars(&dynamic) else {
                continue; // 渲染失败跳过该 SDK（诊断价值低，静默降级不阻断 shell）
            };
            let env_vars: Vec<(String, String)> = rendered.into_iter().collect();
            result.push((
                s.name.clone(),
                SelectedSdk {
                    bins: bins.iter().map(|b| b.to_string_lossy().into_owned()).collect(),
                    env_vars,
                },
            ));
        }
        result
    }

    /// 解析单个 SDK 的项目级环境注入（bin 路径 + extra_vars 渲染）
    ///
    /// 未安装/未注册/fuzzy 失败 → None（诊断经 sink.error 走 stderr，跳过不阻断 shell）
    fn select_sdk_envs(&self, name: &str, version_input: &str) -> Option<SelectedSdk> {
        let sdk = match Sdk::from_str(name) {
            Ok(s) => s,
            Err(_) => {
                self.sink.error(format!("project config: unknown sdk `{name}`, skipped"));
                return None;
            }
        };
        let sdk_conf = match self.config.find_sdk(&sdk) {
            Some(c) => c,
            None => {
                self.sink.error(format!("project config: unregistered sdk `{name}`, skipped"));
                return None;
            }
        };

        // 模糊匹配本地已装版本（与 switch 同源）
        let versions = self.list_local_sdk_versions(&sdk).ok()?;
        if versions.is_empty() {
            self.sink.error(format!(
                "project config: `{name}` has no installed versions, `{version_input}` falls back to global"
            ));
            return None;
        }
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = match fuzzy_match_version_core(&version_strings, version_input) {
            Ok(m) => m,
            Err(e) => {
                self.sink.error(format!("project config: `{name}` {e}"));
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
///
/// `session_override`：`use --shell` 模拟视角优先（进程内尚未 export 该变量）。
/// 供 current.rs 复用（只读查询），改读取规则需两边同步。
pub(crate) fn session_env_value(sdk_name: &str, session_override: Option<(&str, &str)>) -> Option<String> {
    if let Some((_, version)) = session_override.filter(|(n, _)| *n == sdk_name) {
        return Some(version.to_string());
    }
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

/// 渲染可缓存部分：应注入 PATH 条目 + 静态 env 行（unset/export）
///
/// PATH 条目顺序 = selected 迭代序（会话 → 项目 → 全局），前插后最前优先，
/// 保证项目 pin 覆盖全局 symlink、会话覆盖一切。
fn render_env_parts(
    shell: Shell,
    selected: &[(String, SelectedSdk)],
    global_start: usize,
    known: &BTreeSet<String>,
) -> EnvParts {
    let cur: BTreeSet<&str> = selected
        .iter()
        .flat_map(|(_, s)| s.env_vars.iter().map(|(k, _)| k.as_str()))
        .collect();

    // 分层条目：[..global_start] = 会话+项目（top，显式声明）；[global_start..] = 全局（底座）
    let mut top_entries: Vec<String> = Vec::new();
    let mut global_entries: Vec<String> = Vec::new();
    for (idx, (_, s)) in selected.iter().enumerate() {
        let sink = if idx < global_start {
            &mut top_entries
        } else {
            &mut global_entries
        };
        sink.extend(s.bins.iter().cloned());
    }

    // 静态 env 行：unset（known ∖ cur）+ export（cur），语法走 ShellSyntax fn
    let b = shell.syntax();
    let mut lines = Vec::new();
    for key in known.iter().filter(|k| !cur.contains(k.as_str())) {
        lines.push((b.unset_line)(key));
    }
    for (_, s) in selected {
        for (k, v) in &s.env_vars {
            lines.push((b.export_line)(k, v));
        }
    }

    EnvParts {
        top_entries,
        global_entries,
        env_lines: if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        },
    }
}

/// 拼装最终 env 脚本：PATH 外科手术重组 + 静态 env 行（行序：base 自愈 → PATH → INJECTED → env）
fn assemble_env_script(
    shell: Shell,
    cur_path: Option<&str>,
    injected: Option<&str>,
    base_path: Option<&str>,
    top_entries: &[String],
    global_entries: &[String],
    env_lines: &str,
) -> String {
    let final_entries = rebuild_path_entries(
        &split_path_list(cur_path.unwrap_or("")),
        &split_injected(injected.unwrap_or("")),
        top_entries,
        global_entries,
        &split_path_list(base_path.unwrap_or("")),
    );
    let flat_injected: Vec<String> = top_entries.iter().chain(global_entries.iter()).cloned().collect();
    let b = shell.syntax();
    let mut lines = vec![
        (b.base_self_heal_line)(),
        (b.path_line)(&final_entries),
        // 注入记录用单字符串传输（fish 普通 list 导出按空格拼接，禁 list 语法）
        (b.export_line)(ENV_HOOK_INJECTED_PATHS, &flat_injected.join(PATH_LIST_SEPARATOR)),
    ];
    if !env_lines.is_empty() {
        lines.push(env_lines.trim_end_matches('\n').to_string());
    }
    lines.join("\n") + "\n"
}

/// PATH 外科手术重组：剥掉上次注入，本次按**分层语义**归位，其余条目原样保留
///
/// 分层优先级（由层语义决定位置，不依赖历史）：
/// - `top`（会话+项目 = 显式版本声明）→ 顶最前，压过外部前插（venv/conda/手动 export）
/// - `global`（全局底座）→ 贴启动 PATH 快照（base）边界：rest 中首个属于 base 的条目之前，
///   外部前插条目天然保持在 global 之前（#17 语义：venv 压全局 symlink）
/// - base 未锚定（手动 eval）→ global 顶前。每条 injected 只剥一次出现；
///   输出保首现去重（防历史重复条目滚雪球撑爆 Windows 32767 环境变量值上限）。
fn rebuild_path_entries(
    cur: &[String],
    injected: &[String],
    top: &[String],
    global: &[String],
    base: &[String],
) -> Vec<String> {
    let mut pending: Vec<&str> = injected.iter().map(|s| s.as_str()).collect();
    let mut rest: Vec<String> = Vec::with_capacity(cur.len());
    for e in cur {
        if let Some(pos) = pending.iter().position(|p| path_entry_eq(p, e)) {
            pending.swap_remove(pos); // 每条注入只剥一次出现
        } else {
            rest.push(e.clone());
        }
    }
    // global 锚点：rest 中首个属于 base 的条目索引（外部前插条目都排它前面）；base 空 → 0
    let k = if base.is_empty() {
        0
    } else {
        rest.iter().position(|e| base.iter().any(|b| path_entry_eq(b, e))).unwrap_or(0)
    };
    // 不变量：k ≤ rest.len()（position 的返回值域），切片安全
    let mut result = Vec::with_capacity(rest.len() + top.len() + global.len());
    result.extend_from_slice(top);
    result.extend_from_slice(&rest[..k]);
    result.extend_from_slice(global);
    result.extend_from_slice(&rest[k..]);
    dedup_keep_first(result)
}

/// 保首现去重（Windows 键大小写不敏感，与 path_entry_eq 同语义）：PATH 查找命中首个，语义不变
fn dedup_keep_first(entries: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::with_capacity(entries.len());
    entries
        .into_iter()
        .filter(|e| {
            let key = if cfg!(windows) {
                e.to_ascii_lowercase()
            } else {
                e.clone()
            };
            seen.insert(key)
        })
        .collect()
}

/// 拆 PATH 串为条目（空串 = 无条目；中间空段保留——语义为当前目录）
fn split_path_list(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(PATH_LIST_SEPARATOR).map(|s| s.to_string()).collect()
}

/// 拆上次注入记录（滤空段——空串/损坏输入当无记录处理）
fn split_injected(raw: &str) -> Vec<String> {
    raw.split(PATH_LIST_SEPARATOR)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// PATH 条目等价比较：Windows 大小写不敏感，其余平台精确匹配
fn path_entry_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
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
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    /// render_env_parts：按 global_start 分层 + 静态 env 行（无自愈/PATH 行——那由 assemble 实时生成）
    #[test]
    fn render_env_parts_golden() {
        // sample_selected 单 java：global_start=1 → java 归 top 段；global_start=0 → 归 global 段
        let top = render_env_parts(Shell::Bash, &sample_selected(), 1, &sample_known());
        assert_eq!(top.top_entries, s(&["bin/a", "bin/b"]));
        assert!(top.global_entries.is_empty());
        let g = render_env_parts(Shell::Bash, &sample_selected(), 0, &sample_known());
        assert!(g.top_entries.is_empty());
        assert_eq!(g.global_entries, s(&["bin/a", "bin/b"]));
        assert_eq!(top.env_lines, "unset EXTRA\nexport JAVA_HOME=\"/sdk/java\"\n");
    }

    // ============ rebuild_path_entries：分层重组场景穷举（base = 启动快照条目）============

    /// 稳态幂等：注入无变化 → 结果不变（global 贴 base 边界）
    #[test]
    fn rebuild_steady_state_idempotent() {
        let r = rebuild_path_entries(
            &s(&["b1", "b2", "sys"]),
            &s(&["b1", "b2"]),
            &[],
            &s(&["b1", "b2"]),
            &s(&["sys"]),
        );
        assert_eq!(r, s(&["b1", "b2", "sys"]));
    }

    /// 外部前插（#17 本体：venv activate）→ 前插条目保留且优先于 global
    #[test]
    fn rebuild_global_yields_to_external() {
        let r = rebuild_path_entries(&s(&["venv", "b", "sys"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r, s(&["venv", "b", "sys"]));
        // 多层外部前插（conda 再 venv）同样保留
        let r2 = rebuild_path_entries(&s(&["v1", "v2", "b", "sys"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r2, s(&["v1", "v2", "b", "sys"]));
    }

    /// pin 存在（top 段）：store 压 venv、venv 压 global（分层优先级）
    #[test]
    fn rebuild_top_overrides_external() {
        // cur = pin 顶前后的状态：[store, links, venv, sys]
        let r = rebuild_path_entries(
            &s(&["store", "links", "venv", "sys"]),
            &s(&["store", "links"]),
            &s(&["store"]),
            &s(&["links"]),
            &s(&["sys"]),
        );
        assert_eq!(r, s(&["store", "venv", "links", "sys"]));
    }

    /// pin 删除（top 段消失，本轮用户反馈 bug）：venv 回到 global 之前
    #[test]
    fn rebuild_top_removed_restores_external() {
        let r = rebuild_path_entries(
            &s(&["store", "links", "venv", "sys"]),
            &s(&["store", "links"]),
            &[],
            &s(&["links"]),
            &s(&["sys"]),
        );
        assert_eq!(r, s(&["venv", "links", "sys"]));
    }

    /// 会话层 use --shell（top）：顶到最前（LIFO，压过外部前插）
    #[test]
    fn rebuild_top_session_lifts_to_front() {
        let r = rebuild_path_entries(&s(&["venv", "old", "sys"]), &s(&["old"]), &s(&["new"]), &[], &s(&["sys"]));
        assert_eq!(r, s(&["new", "venv", "sys"]));
    }

    /// injected 缺失（首跑/存量升级）：global 仍贴 base 边界；旧注入残留经去重一次收敛
    #[test]
    fn rebuild_missing_injected_converges() {
        // cur 里 b 是旧版无记录注入（剥不掉、在 base 前）→ global 插 base 边界 + 去重收敛
        let r = rebuild_path_entries(&s(&["b", "sys"]), &[], &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r, s(&["b", "sys"]));
        // 收敛后幂等
        let r2 = rebuild_path_entries(&s(&["b", "sys"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r2, s(&["b", "sys"]));
    }

    /// 历史重复条目（注册表持久化 + hook 注入并存）：保首现去重，PATH 越用越短直至收敛
    #[test]
    fn rebuild_dedups_repeated_entries() {
        let r = rebuild_path_entries(
            &s(&["b", "sys1", "b", "sys2", "b"]),
            &s(&["b"]),
            &[],
            &s(&["b"]),
            &s(&["sys1", "sys2"]),
        );
        assert_eq!(r, s(&["b", "sys1", "sys2"]));
        // 外部重复（用户/venv 加的）同样去重（查找命中首个，语义不变）
        let r2 = rebuild_path_entries(&s(&["v", "sys", "v"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r2, s(&["v", "b", "sys"]));
    }

    /// 用户手动追加 PATH（base 之后）：保留在后
    #[test]
    fn rebuild_preserves_appended_external() {
        let r = rebuild_path_entries(&s(&["b", "sys", "u"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r, s(&["b", "sys", "u"]));
    }

    /// base 未锚定（手动 eval，无 hook）：global 顶前（与前插语义一致）
    #[test]
    fn rebuild_no_base_prepends_global() {
        let r = rebuild_path_entries(&s(&["venv", "b", "sys"]), &s(&["b"]), &[], &s(&["b"]), &[]);
        assert_eq!(r, s(&["b", "venv", "sys"]));
    }

    /// base 首条目不在 cur 中（用户重排/删除）：global 回退顶前（sdkm 管理条目必须存在）
    #[test]
    fn rebuild_base_missing_prepends_global() {
        let r = rebuild_path_entries(&s(&["venv", "x"]), &s(&["b"]), &[], &s(&["b"]), &s(&["sys"]));
        assert_eq!(r, s(&["b", "venv", "x"]));
    }

    /// 空 PATH / 空注入：直接注入
    #[test]
    fn rebuild_empty_cur() {
        let r = rebuild_path_entries(&[], &[], &[], &s(&["b"]), &[]);
        assert_eq!(r, s(&["b"]));
    }

    /// 拆分工具：PATH 空串 = 无条目、中间空段保留（当前目录语义）；injected 滤空（空串当无记录）
    #[test]
    fn split_tools_semantics() {
        assert_eq!(split_path_list(""), Vec::<String>::new());
        assert_eq!(
            split_path_list(&format!("a{}{}b", PATH_LIST_SEPARATOR, PATH_LIST_SEPARATOR)),
            s(&["a", "", "b"])
        );
        assert_eq!(split_injected(""), Vec::<String>::new());
        assert_eq!(
            split_injected(&format!("a{}{}b", PATH_LIST_SEPARATOR, PATH_LIST_SEPARATOR)),
            s(&["a", "b"])
        );
    }

    /// Windows 路径大小写不敏感匹配（仅 Windows 测试机跑）
    #[cfg(windows)]
    #[test]
    fn rebuild_windows_case_insensitive() {
        let r = rebuild_path_entries(
            &s(&["D:\\LINKS\\java\\bin", "sys"]),
            &s(&["d:\\links\\java\\bin"]),
            &[],
            &s(&["n"]),
            &s(&["sys"]),
        );
        assert_eq!(r, s(&["n", "sys"]));
    }

    // ===================== assemble_env_script：四 shell golden =====================

    /// bash/zsh：base 自愈 → PATH 整串 → INJECTED 记录 → 静态 env 行
    ///
    /// cur 输入按平台分隔符拆（sdkm 进程视角），PATH 行输出固定 `:`（bash 语法语义）。
    /// top=["new"]（会话/项目层）顶最前压过 venv。
    #[test]
    fn assemble_bash_golden() {
        let sep = PATH_LIST_SEPARATOR;
        let script = assemble_env_script(
            Shell::Bash,
            Some(&format!("venv{sep}old{sep}sys")),
            Some("old"),
            Some("sys"),
            &s(&["new"]),
            &[],
            "unset EXTRA\nexport JAVA_HOME=\"/sdk/java\"\n",
        );
        assert_eq!(
            script,
            format!(
                "[ -z \"${{{e}:-}}\" ] && export {e}=\"$PATH\"\n\
                 export PATH=\"new:venv:sys\"\n\
                 export {ip}=\"new\"\n\
                 unset EXTRA\n\
                 export JAVA_HOME=\"/sdk/java\"\n",
                e = ENV_HOOK_BASE_PATH,
                ip = ENV_HOOK_INJECTED_PATHS,
            )
        );
        // zsh 语法与 bash 相同（同 backend fn）
        assert_eq!(
            script,
            assemble_env_script(
                Shell::Zsh,
                Some(&format!("venv{sep}old{sep}sys")),
                Some("old"),
                Some("sys"),
                &s(&["new"]),
                &[],
                "unset EXTRA\nexport JAVA_HOME=\"/sdk/java\"\n",
            )
        );
    }

    /// fish：PATH 逐条目引号；INJECTED 单字符串（list 导出按空格拼接，禁 list 语法）
    #[test]
    fn assemble_fish_golden() {
        let sep = PATH_LIST_SEPARATOR;
        let script = assemble_env_script(
            Shell::Fish,
            Some(&format!("venv{sep}old{sep}sys")),
            Some("old"),
            Some("sys"),
            &s(&["new"]),
            &[],
            "",
        );
        assert_eq!(
            script,
            format!(
                "if not set -q {e}\n    set -gx {e} $PATH\nend\n\
                 set -gx PATH \"new\" \"venv\" \"sys\"\n\
                 set -gx {ip} \"new\"\n",
                e = ENV_HOOK_BASE_PATH,
                ip = ENV_HOOK_INJECTED_PATHS,
            )
        );
    }

    /// PowerShell：分号整串（PATH_LIST_SEPARATOR 随编译平台）
    #[test]
    fn assemble_powershell_golden() {
        let sep = PATH_LIST_SEPARATOR;
        let script = assemble_env_script(
            Shell::PowerShell,
            Some(&format!("venv{sep}old{sep}sys")),
            Some("old"),
            Some("sys"),
            &s(&["new"]),
            &[],
            "",
        );
        assert_eq!(
            script,
            format!(
                "if (-not $env:{e}) {{ $env:{e} = $env:PATH }}\n\
                 $env:PATH = \"new{sep}venv{sep}sys\"\n\
                 $env:{ip} = \"new\"\n",
                e = ENV_HOOK_BASE_PATH,
                ip = ENV_HOOK_INJECTED_PATHS,
            )
        );
    }

    /// 条目未变（global 段稳定，#17 场景）：venv 保持最前
    #[test]
    fn assemble_anchors_when_entries_unchanged() {
        let sep = PATH_LIST_SEPARATOR;
        let script = assemble_env_script(
            Shell::Bash,
            Some(&format!("venv{sep}bin{sep}sys")),
            Some("bin"),
            Some("sys"),
            &[],
            &s(&["bin"]),
            "",
        );
        assert!(
            script.contains("export PATH=\"venv:bin:sys\""),
            "global 段应贴 base 边界（venv 优先），got: {script}"
        );
    }

    /// 条目仅减少（离开项目/卸载）：venv 保持在 global 之前
    #[test]
    fn assemble_anchors_when_entries_only_removed() {
        let sep = PATH_LIST_SEPARATOR;
        let script = assemble_env_script(
            Shell::Bash,
            Some(&format!("venv{sep}keep{sep}gone{sep}sys")),
            Some(&format!("keep{sep}gone")),
            Some("sys"),
            &[],
            &s(&["keep"]),
            "",
        );
        assert!(
            script.contains("export PATH=\"venv:keep:sys\""),
            "仅减少应保持 venv 优先，got: {script}"
        );
    }

    /// 空 cur + 空 bins：PATH 行回退 base 引用（手动 eval 场景的自愈兜底）
    #[test]
    fn assemble_empty_falls_back_to_base() {
        let e = ENV_HOOK_BASE_PATH;
        let script = assemble_env_script(Shell::Bash, None, None, None, &[], &[], "");
        assert_eq!(
            script,
            format!(
                "[ -z \"${{{e}:-}}\" ] && export {e}=\"$PATH\"\nexport PATH=\"${e}\"\nexport {ip}=\"\"\n",
                ip = ENV_HOOK_INJECTED_PATHS
            )
        );
    }
}
