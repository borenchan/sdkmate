//! shell hook 缓存：`sdkm env` 高频路径的「零磁盘解析」加速层。
//!
//! 缓存文件 `<sdkm_home>/.cache/hook_cache.json`，结构
//! `{ "<PWD 绝对路径>": { config_path, mtime_nanos, env_script, ... } }`。
//! 新鲜度判据 = 命中的 `.sdkm.toml` 的 mtime（纳秒，防同秒编辑盲区）+ shell +
//! schema 版本 + 会话指纹（`SDKM_ACTIVE_*` 变量集合——中途 `use --shell` 改会话
//! 变量必须触发重算，否则吐旧脚本压过会话层）：
//! 全部不变 → 直接吐缓存里的完整 env_script；任一变了 → 重新解析并回写。
//! 损坏自愈：解析失败删缓存文件当空表。全程 best-effort，读写失败静默
//! 降级到实时解析（缓存纯优化，不影响正确性）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use util::consts::SDKM_SESSION_ENV_PREFIX;
use util::path::get_hook_cache_path;

/// 缓存 schema 版本：脚本模板/生成逻辑/新鲜度判据变化时 bump，旧缓存条目整体失效（自动重建）
pub const CACHE_SCHEMA_VERSION: u128 = 4;

/// 单条 hook 缓存：命中的配置文件路径 + 采样时 mtime（纳秒）+ 已生成好的完整 env 脚本
#[derive(Serialize, Deserialize, Default)]
pub struct HookEntry {
    /// 命中的 `.sdkm.toml` 绝对路径（无项目配置时空串 = 锚定「无配置」状态）
    pub config_path: String,
    /// 纳秒 mtime（同秒连续编辑也能感知变化——热更新验收点）
    pub mtime_nanos: u128,
    /// 生成好的完整 eval 脚本（PATH 重建 + unset + export），命中直接吐
    pub env_script: String,
    /// 生成此条目时的 schema 版本（不匹配 = 旧格式，当 miss 重建）
    #[serde(default)]
    pub schema_version: u128,
    /// 生成此条目的 shell 判别序号（`Shell as u8`）。命中时 shell 不符当 miss——
    /// 防「同一 PWD 跨 --shell 拿到上一 shell 脚本」的串扰（key 仍只含 PWD，不破坏 prune）
    #[serde(default)]
    pub shell: u8,
    /// 生成此条目时的会话指纹（所有 `SDKM_ACTIVE_*` 变量按名排序拼接）。命中时与
    /// 当前进程指纹不符当 miss——中途 `use --shell` 改会话变量后必须重算，
    /// 否则缓存吐旧脚本压过会话层（会话 > 项目的优先级文档会被缓存打破）
    #[serde(default)]
    pub session_fingerprint: String,
}

/// 缓存表：PWD 绝对路径字符串 → hook 条目（transparent 序列化为裸 JSON object）
#[derive(Serialize, Deserialize, Default)]
#[serde(transparent)]
struct HookMap(HashMap<String, HookEntry>);

pub struct HookCache {
    map: HookMap,
    dirty: bool,
}

impl HookCache {
    /// 加载缓存：路径错 → 空；文件损坏 → 删文件当空（自愈，防反复撞同一坏文件）
    pub fn load() -> Self {
        let path = match get_hook_cache_path() {
            Ok(p) => p,
            Err(_) => return Self::empty(),
        };
        let data = fs::read(&path).unwrap_or_default();
        match serde_json::from_slice::<HookMap>(&data) {
            Ok(map) => HookCache { map, dirty: false },
            Err(_) => {
                // 损坏：删文件自愈（size_cache 只当空表，这里按任务书 2.5 显式删）
                let _ = fs::remove_file(&path);
                Self::empty()
            }
        }
    }

    fn empty() -> Self {
        HookCache {
            map: HookMap::default(),
            dirty: false,
        }
    }

    /// 查缓存是否命中：PWD 有记录、shell 相符、schema 版本一致、会话指纹一致、
    /// 且其锚定的配置文件 mtime 未变
    ///
    /// 返回命中条目的 env_script 引用；未命中/无记录/shell 不符/schema 不符/
    /// 指纹不符/mtime 变了 → None（触发实时解析）。
    pub fn resolve(&self, pwd: &Path, shell: u8) -> Option<&HookEntry> {
        let key = pwd.to_string_lossy();
        let entry = self.map.0.get(key.as_ref())?;
        if entry.shell != shell {
            return None;
        }
        if entry.schema_version != CACHE_SCHEMA_VERSION {
            return None;
        }
        if entry.session_fingerprint != current_session_fingerprint() {
            return None;
        }
        if entry.config_path.is_empty() {
            return Some(entry);
        }
        let current = current_mtime_nanos(Path::new(&entry.config_path))?;
        if entry.mtime_nanos == current {
            Some(entry)
        } else {
            None
        }
    }

    /// 插入/更新一条缓存（dirty=true，save 时落盘）
    pub fn put(&mut self, pwd: &Path, entry: HookEntry) {
        let key = pwd.to_string_lossy().into_owned();
        self.map.0.insert(key, entry);
        self.dirty = true;
    }

    /// 清理指向不存在目录的孤儿条目（目录被删后残留的失效 key，惰性自愈）
    pub fn prune(&mut self) {
        let before = self.map.0.len();
        self.map.0.retain(|k, _| Path::new(k).exists());
        if self.map.0.len() != before {
            self.dirty = true;
        }
    }

    /// dirty 时原子写（tmp+rename），失败静默
    pub fn save(&self) {
        if !self.dirty {
            return;
        }
        let path = match get_hook_cache_path() {
            Ok(p) => p,
            Err(_) => return,
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        let Ok(json) = serde_json::to_vec(&self.map) else {
            return;
        };
        if fs::write(&tmp, &json).is_err() {
            return;
        }
        let _ = fs::rename(&tmp, &path);
    }
}

/// 文件 mtime（纳秒，距 UNIX_EPOCH）；取不到返 None → 当 cache miss（实时解析）
fn current_mtime_nanos(path: &Path) -> Option<u128> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_nanos())
}

/// 当前进程所有 `SDKM_ACTIVE_*` 会话变量拼成的指纹（按变量名排序，`name=value` 分号连接）
///
/// 无会话变量 → 空串。用作缓存新鲜度判据：会话变量 set/改/unset 任一变化都会
/// 改变指纹 → 缓存 miss → 重算。注意指纹判据只保证「变了必重算」，不要求跨版本
/// 稳定（旧条目由 schema bump 兜底整体失效）。
pub fn current_session_fingerprint() -> String {
    let mut pairs: Vec<String> = env::vars()
        .filter(|(k, _)| k.starts_with(SDKM_SESSION_ENV_PREFIX))
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    pairs.sort();
    pairs.join(";")
}

/// 供外部获取指定文件的纳秒 mtime（生成缓存条目时锚定用）
pub fn file_mtime_nanos(path: &Path) -> Option<u128> {
    current_mtime_nanos(path)
}
