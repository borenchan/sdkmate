// ──────────────────────────────────────────────────────
// 配置快照与回滚基础设施
// ──────────────────────────────────────────────────────

use anyhow::{Context, Result};
use std::fs;
use util::path::get_sdkm_config_path;

use super::SdkmConfig;

/// config 操作前的配置快照，用于写入失败时回滚
pub struct ConfigSnapshot {
    /// 内存级完整配置副本（SdkmConfig 已 derive Clone）
    pub(crate) old_config: SdkmConfig,
    /// 磁盘级原始 TOML 内容（最坏情况的磁盘级恢复）
    pub(crate) old_file_content: String,
}

/// 拍摄配置快照：在修改配置之前备份
pub fn take_config_snapshot() -> Result<ConfigSnapshot> {
    let old_config = SdkmConfig::read_from_disk()?;
    let config_path = get_sdkm_config_path()?;
    let old_file_content = fs::read_to_string(&config_path)
        .context("Failed to read current config for snapshot")?;
    Ok(ConfigSnapshot {
        old_config,
        old_file_content,
    })
}

/// 回滚配置：优先尝试内存级恢复（clone + atomic_write）
pub fn rollback_config(snapshot: &ConfigSnapshot) -> Result<()> {
    snapshot.old_config.atomic_write_to_disk()
}

/// 最坏情况回滚：直接用旧文件内容恢复磁盘文件
/// 仅在内存级恢复失败时使用（如序列化异常）
pub fn rollback_config_from_raw(snapshot: &ConfigSnapshot) -> Result<()> {
    let config_path = get_sdkm_config_path()?;
    fs::write(&config_path, &snapshot.old_file_content)
        .context("Failed to restore config from raw backup")?;
    Ok(())
}
