// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境 + 删除 home 目录内容
//
// 复用 cleanup_sdk_environment（清单个 SDK 的 symlink/PATH/env/current）逐个清理激活的 SDK。
// binary 本身不自删（Windows 上 running exe 被锁不可靠），提示用户手动删除。

use crate::manager::SdkManager;
use anyhow::{Result, bail};
use std::str::FromStr;
use util::path::get_sdkm_home;
use util::sdk::Sdk;
use util::terminal::prompt_confirm;
use util::{info, success, warning};

impl SdkManager {
    /// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境并删除 home 目录内容
    ///
    /// - 遍历 config 中所有激活的 SDK，复用 `cleanup_sdk_environment` 清其 symlink/PATH/env/current
    /// - 删除 home 目录内容（store/links/.tmp/cache/config.toml），尽力而为
    /// - binary 本身与 PATH 条目提示用户手动清理（running exe 自删跨平台不可靠）
    pub fn uninstall_self(&mut self, yes: bool) -> Result<()> {
        if !yes {
            let confirmed = prompt_confirm(
                "This will clean up ALL managed SDK environments (symlink/PATH/env/current) and remove the sdkm home directory (store/links/config/cache). The sdkm binary itself and any PATH entry must be removed manually. Continue?",
            )?;
            if !confirmed {
                bail!("Self-uninstall cancelled by user");
            }
        }

        // 收集激活的 SDK（owned，避免借 self.config 与后续 &mut self 冲突）
        let active_sdks: Vec<Sdk> = self
            .config
            .sdks
            .iter()
            .filter(|c| c.current_version.is_some())
            .filter_map(|c| Sdk::from_str(&c.name).ok())
            .collect();

        info!("cleaning up {} managed SDK environment(s)...", active_sdks.len());
        for sdk in &active_sdks {
            // 复用 cleanup_sdk_environment：删 symlink + remove PATH + unset env + current=None
            if let Err(e) = self.cleanup_sdk_environment(sdk) {
                warning!("failed to clean up `{}` environment: {}", sdk, e);
            }
        }

        // 删除 home 目录内容（store/links/.tmp/cache/config.toml），尽力而为
        let home = get_sdkm_home()?;
        info!("removing sdkm home contents at {}", home.display());
        for sub in ["store", "links", ".tmp", "cache"] {
            let p = home.join(sub);
            if p.exists()
                && let Err(e) = std::fs::remove_dir_all(&p)
            {
                warning!("failed to remove `{}`: {}", p.display(), e);
            }
        }
        let cfg = home.join("config.toml");
        if cfg.exists()
            && let Err(e) = std::fs::remove_file(&cfg)
        {
            warning!("failed to remove `{}`: {}", cfg.display(), e);
        }

        // binary 本身：running exe 跨平台不可靠自删（Windows 锁），提示用户手动删
        if let Ok(exe) = std::env::current_exe() {
            warning!(
                "sdkm binary remains at `{}` (cannot self-remove while running); please remove it manually",
                exe.display()
            );
        }
        info!("if sdkm's directory is in your PATH, remove the entry manually");

        success!("sdkm self-uninstall complete (binary/path remain to remove manually)");
        Ok(())
    }
}
