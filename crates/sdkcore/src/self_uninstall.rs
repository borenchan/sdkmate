// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境 + 删除 home 目录内容
//
// 复用 cleanup_sdk_environment（清单个 SDK 的 symlink/PATH/env/current）逐个清理激活的 SDK。
// binary 本身不自删（Windows 上 running exe 被锁不可靠），提示用户手动删除。
//
// 交互确认由 CLI 层（SelfUninstallHandler）负责，core 只做业务，便于测试直接调用无需应答 stdin。

use crate::manager::SdkManager;
use anyhow::Result;
use std::str::FromStr;
use util::consts::{CONFIG_FILE_NAME, SDKM_CACHE_DIR, SDKM_LINKS_DIR, SDKM_STORE_DIR, SDKM_TMP_DIR};
use util::path::get_sdkm_home;
use util::sdk::Sdk;
use util::{info, success, warning};

impl SdkManager {
    /// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境并删除 home 目录内容
    ///
    /// - 遍历 config 中所有激活的 SDK，复用 `cleanup_sdk_environment` 清其 symlink/PATH/env/current
    /// - 删除 home 目录内容（store/links/.tmp/cache/config.toml），尽力而为
    /// - binary 本身与 PATH 条目提示用户手动清理（running exe 自删跨平台不可靠）
    ///
    /// 确认提示由 CLI 层负责（破坏性操作必须交互确认），core 不含交互逻辑。
    pub fn uninstall_self(&mut self) -> Result<()> {
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
        for sub in [SDKM_STORE_DIR, SDKM_LINKS_DIR, SDKM_TMP_DIR, SDKM_CACHE_DIR] {
            let p = home.join(sub);
            if p.exists()
                && let Err(e) = std::fs::remove_dir_all(&p)
            {
                warning!("failed to remove `{}`: {}", p.display(), e);
            }
        }
        let cfg = home.join(CONFIG_FILE_NAME);
        if cfg.exists()
            && let Err(e) = std::fs::remove_file(&cfg)
        {
            warning!("failed to remove `{}`: {}", cfg.display(), e);
        }

        // sdkm 自身的 PATH 条目：自动移除（binary 保留但 PATH 条目应清理，尽力而为）
        // sdkm 目录可能不在 PATH 中（remove_sdk_path 幂等：无该条目则静默）
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let exe_dir_str = exe_dir.to_string_lossy().to_string();
                if let Err(e) = self.env_operation.remove_sdk_path(&exe_dir_str) {
                    warning!("failed to remove sdkm PATH entry `{}`: {}", exe_dir_str, e);
                }
            }
            // binary 本身：running exe 跨平台不可靠自删（Windows 锁），提示用户手动删
            warning!(
                "sdkm binary remains at `{}` (cannot self-remove while running); please remove it manually",
                exe.display()
            );
        }

        success!("sdkm self-uninstall complete (binary remains to remove manually)");
        Ok(())
    }
}
