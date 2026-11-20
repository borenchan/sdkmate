// 清理缓存与临时目录：`.cache`（版本 API / 体积缓存）与 `.tmp`（安装、自更新临时文件）。
//
// 不触碰 store/links/config.toml。被删目录按需自动重建：版本缓存重新拉取、
// size 缓存下次扫描重算（首查变慢属预期）。单项删除失败（如运行中 GUI 锁定的
// `.discard` 副本）发 warning 不中断，尽力清理。
//
// GUI 诊断页「清除缓存数据」与 CLI `sdkm clean` 共用本 API（核心逻辑归一处）。

use crate::manager::SdkManager;
use crate::size_cache::dir_size_parallel;
use anyhow::{Context, Result};
use sdkm_util::consts::{SDKM_CACHE_DIR, SDKM_TMP_DIR};
use sdkm_util::path::get_sdkm_home;
use std::fs;
use std::path::Path;

/// 清理结果（释放字节数）
#[derive(Debug, Clone, Default)]
pub struct CleanReport {
    /// `.cache` 释放字节数
    pub cache_bytes: u64,
    /// `.tmp` 释放字节数
    pub tmp_bytes: u64,
}

impl CleanReport {
    pub fn total(&self) -> u64 {
        self.cache_bytes + self.tmp_bytes
    }
}

impl SdkManager {
    /// 清理缓存与临时目录（want_cache / want_tmp 至少一真，由调用端按参数保证）
    ///
    /// 目录本身保留，只删内容；不存在视为 0 字节。
    pub fn clean(&self, want_cache: bool, want_tmp: bool) -> Result<CleanReport> {
        let home = get_sdkm_home()?;
        let mut report = CleanReport::default();
        if want_cache {
            report.cache_bytes = self.clean_dir_contents(&home.join(SDKM_CACHE_DIR))?;
        }
        if want_tmp {
            report.tmp_bytes = self.clean_dir_contents(&home.join(SDKM_TMP_DIR))?;
        }
        Ok(report)
    }

    /// 清空单个目录的内容（保留目录本身；逐项尽力删除）
    fn clean_dir_contents(&self, dir: &Path) -> Result<u64> {
        if !dir.exists() {
            return Ok(0);
        }
        self.sink.info(format!("cleaning `{}`...", dir.display()));
        let entries = fs::read_dir(dir).context(format!("Failed to read `{}`", dir.display()))?;
        let mut freed = 0u64;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            // 先算体积再删（删除后无从统计）
            let size = dir_size_parallel(&path).unwrap_or(0);
            let removed = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            match removed {
                Ok(()) => freed += size,
                Err(e) => self
                    .sink
                    .warning(format!("failed to remove `{}`: {e} (skipped)", path.display())),
            }
        }
        Ok(freed)
    }
}
