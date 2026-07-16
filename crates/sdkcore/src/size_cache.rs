//! SDK 版本目录 size 缓存：纯 ls 侧读时 Concern，install/uninstall/switch 不碰（解耦）。
//!
//! 缓存文件 `<sdkm_home>/.cache/size.json`，结构 `{ "<dir 路径>": { bytes, mtime_secs } }`。
//! 新鲜度判据 = 目录 mtime（直接子项增删才变）：SDK 版本目录装完即冻结 → 永久命中；
//! 外部手动改目录直接子项 → mtime 变 → 失效重算。原子写（tmp+rename），全程 best-effort
//! （缓存纯优化，读不到/写不进都不影响正确性，只是退化到现场计算）。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use util::path::get_size_cache_path;

/// 单条 size 缓存：字节数 + 采样时的目录 mtime（秒）
#[derive(Serialize, Deserialize, Default)]
struct SizeEntry {
    bytes: u64,
    mtime_secs: u64,
}

/// 缓存表：路径字符串 → size 条目（transparent 序列化为裸 JSON object）
#[derive(Serialize, Deserialize, Default)]
#[serde(transparent)]
struct SizeMap(HashMap<String, SizeEntry>);

pub struct SizeCache {
    map: SizeMap,
    dirty: bool,
}

impl SizeCache {
    /// 加载缓存：文件不存在/损坏/路径错 → 空（best-effort，不阻断 ls）
    pub fn load() -> Self {
        let path = match get_size_cache_path() {
            Ok(p) => p,
            Err(_) => return Self::empty(),
        };
        let data = fs::read(&path).unwrap_or_default();
        let map = serde_json::from_slice::<SizeMap>(&data).unwrap_or_default();
        SizeCache { map, dirty: false }
    }

    fn empty() -> Self {
        SizeCache {
            map: SizeMap::default(),
            dirty: false,
        }
    }

    /// 仅查缓存是否命中（mtime 一致）：命中返 Some(bytes)，未命中/无记录返 None（不触发计算）
    ///
    /// 用于冷热路径判定——全部命中则直接打印最终表，有未命中才走渐进渲染。
    pub fn cached(&self, dir: &Path) -> Option<u64> {
        let key = dir.to_string_lossy();
        let mtime = current_mtime_secs(dir)?;
        let entry = self.map.0.get(key.as_ref())?;
        if entry.mtime_secs == mtime {
            Some(entry.bytes)
        } else {
            None
        }
    }

    /// 取目录大小：命中返缓存值；未命中并行 walk 算 + 回写（dirty=true）
    pub fn resolve(&mut self, dir: &Path) -> u64 {
        if let Some(bytes) = self.cached(dir) {
            return bytes;
        }
        let bytes = dir_size_parallel(dir).unwrap_or(0);
        if let Some(mtime) = current_mtime_secs(dir) {
            let key = dir.to_string_lossy().into_owned();
            self.map.0.insert(
                key,
                SizeEntry {
                    bytes,
                    mtime_secs: mtime,
                },
            );
            self.dirty = true;
        }
        bytes
    }

    /// 清理指向不存在目录的孤儿条目（uninstall 后残留的失效 key，惰性自愈）
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
        let path = match get_size_cache_path() {
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

/// 目录 mtime（秒，距 UNIX_EPOCH）；取不到返 None → 当 cache miss（现场计算）
fn current_mtime_secs(path: &Path) -> Option<u64> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// 并行递归计算目录总大小（jwalk + rayon 工作窃取）
///
/// 不跟随符号链接（follow_links=false）；符号链接条目 is_file()=false 不计入。
/// 仅 size_cache 内部使用（冷路径未命中时触发）。
fn dir_size_parallel(path: &Path) -> Result<u64> {
    use jwalk::WalkDir;
    let total: u64 = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();
    Ok(total)
}
