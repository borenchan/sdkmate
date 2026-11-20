// `sdkm clean`：清理缓存与临时目录（用户主动回收磁盘空间）
//
// 只清 `.cache`（版本 API / 体积缓存）与 `.tmp`（安装、自更新临时文件），
// 不碰 store/links/config.toml；被删内容按需自动重建。
// 不带参数 = 两者都清；`--cache` / `--tmp` 单独清理。

use crate::CommandHandler;
use crate::terminal_sink::manager;
use crate::{detail, info, success};
use clap::Parser;
use sdkm_util::consts::{SIZE_GB, SIZE_KB, SIZE_MB};

/// 字节数 → 人类可读（1.24 GB / 523 MB / 12 KB）
fn human_size(bytes: u64) -> String {
    const KB: f64 = SIZE_KB as f64;
    const MB: f64 = SIZE_MB as f64;
    const GB: f64 = SIZE_GB as f64;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[derive(Debug, Parser)]
pub struct CleanHandler {
    /// Clean only the cache directory (.cache: version API cache, size cache)
    #[arg(long)]
    cache: bool,

    /// Clean only the temp directory (.tmp: downloads, extraction, self-update leftovers)
    #[arg(long)]
    tmp: bool,
}

impl CommandHandler for CleanHandler {
    fn run(&self) -> anyhow::Result<()> {
        let manager = manager()?;
        // 不带参数 = 全部清理；单给一个参数 = 只清那一项
        let (want_cache, want_tmp) = if self.cache || self.tmp {
            (self.cache, self.tmp)
        } else {
            (true, true)
        };

        info!("cleaning sdkm cache and temporary data...");
        let report = manager.clean(want_cache, want_tmp)?;

        if want_cache {
            detail!("cache (.cache): freed {}", human_size(report.cache_bytes));
        }
        if want_tmp {
            detail!("temp  (.tmp):   freed {}", human_size(report.tmp_bytes));
        }
        success!("clean complete, freed {} in total", human_size(report.total()));
        Ok(())
    }
}
