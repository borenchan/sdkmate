// 卸载本地已安装的 SDK 版本
//
// 复用既有逻辑（跨模块调用的均为已 pub 的公共 API，无需抽取通用文件）：
// - list_local_sdk_versions / fuzzy_match_version_core：版本扫描 + 模糊匹配（与 install/switch 同源）
// - switch_sdk_to_version：卸载当前版本且有其他版本时，复用 switch 流程切到其他版本（业务编排）
// - remove_symlink / EnvOperation trait 方法 / config 快照回滚：底层公共 API

use crate::config::{rollback_config, take_config_snapshot};
use crate::link::symlink::remove_symlink;
use crate::manager::SdkManager;
use crate::version::fuzzy_match_version_core;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use util::path::get_installed_sdks_dir;
use util::sdk::Sdk;
use util::terminal::prompt_confirm;
use util::{detail, info, success, warning};

impl SdkManager {
    /// 卸载本地已安装的 SDK 版本
    ///
    /// - 非当前激活版本：仅删 `store/<sdk>/<version>` 目录
    /// - 当前激活版本且有其他已装版本：先 switch 到其他版本再删（复用 switch 流程）
    /// - 当前激活版本且仅此一版：清理 symlink/PATH/env/current 后再删
    ///
    /// 卸载前一次性确认，文案说明 sdkm 将如何处理；`yes=true` 跳过该确认。
    pub fn uninstall_sdk(&mut self, sdk: &Sdk, version_input: &str, yes: bool) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        if versions.is_empty() {
            bail!("No installed versions for `{}`. Nothing to uninstall.", sdk);
        }

        // 模糊匹配本地已装版本（复用 fuzzy 核心，找不到匹配内部带 did-you-mean）
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = fuzzy_match_version_core(&version_strings, version_input)?;
        let target_version = matched.full_version;

        // 定位目标版本项
        let target_item = versions.iter().find(|v| v.sdk_version == target_version).context(format!(
            "local not found `{}` version `{}`, please check store dir",
            sdk, target_version
        ))?;
        let target_sdk_dir = target_item.sdk_dir.clone();
        let is_active = target_item.is_active;

        // active 时是否有其他已装版本（取第一个，不做"最新"计算——提示告知具体版本号，
        // 用户不满意可取消后手动 switch）
        let other_version: Option<String> = if is_active {
            versions
                .iter()
                .find(|v| v.sdk_version != target_version)
                .map(|v| v.sdk_version.clone())
        } else {
            None
        };

        // 卸载前一次性确认：告知 sdkm 将如何处理（直接删 / 自动切换另一版本 / 清理环境）
        if !yes {
            let resolved = if matched.fuzzy_matched {
                format!("Resolved '{}' → '{}'. ", version_input, target_version)
            } else {
                String::new()
            };
            let action = match (&other_version, is_active) {
                (Some(other), true) => format!(
                    "`{}` `{}` is the active version. sdkm will switch to `{}` first, then uninstall.",
                    sdk, target_version, other
                ),
                (None, true) => format!(
                    "`{}` `{}` is the only installed version. sdkm will clean up symlink, PATH, env vars and current_version, then remove it.",
                    sdk, target_version
                ),
                _ => format!(
                    "sdkm will uninstall `{}` `{}` (remove the version directory only).",
                    sdk, target_version
                ),
            };
            if !prompt_confirm(&format!("{resolved}{action} Continue?"))? {
                bail!("Uninstall cancelled by user");
            }
        }

        info!("uninstalling `{}` `{}`...", sdk, target_version);

        if is_active {
            match &other_version {
                Some(other) => {
                    info!("switching to {} {} before uninstall...", sdk, other);
                    // switch 重建 symlink 指向新版本，PATH 加的是 symlink 路径非实路径，删旧版本目录安全无残留
                    self.switch_sdk_to_version(sdk, other)?;
                }
                None => self.cleanup_sdk_environment(sdk)?,
            }
        }

        // 删除目标版本目录（失败不中断：环境已清理/switch 已完成，残留目录仅占磁盘，告知用户手动处理）
        let removed = self.remove_version_dir(&target_sdk_dir)?;

        // 清理空的 store/<sdk> 目录
        self.cleanup_empty_sdk_store_dir(sdk)?;

        if removed {
            success!("uninstall `{}` version `{}` success!", sdk, target_version);
        } else {
            warning!(
                "uninstall `{}` `{}` partially complete: version directory remains at `{}` (may be locked by another process or antivirus), please remove it manually",
                sdk,
                target_version,
                target_sdk_dir.display()
            );
        }
        Ok(())
    }

    /// 清除 SDK 的全部激活环境（卸载当前唯一版本时调用）
    ///
    /// 尽力而为：symlink/PATH/env 清理失败仅 warning 不中断后续步骤；
    /// 仅 config（current_version=None）写入失败时回滚并 bail（避免残留指向已删版本的幽灵 current）。
    fn cleanup_sdk_environment(&mut self, sdk: &Sdk) -> Result<()> {
        info!("cleaning up `{}` environment (symlink/PATH/env/current_version)...", sdk);
        let symlink_root = self.config.resolved_symlink_dir()?;
        let sdk_symlink_dir = PathBuf::from(symlink_root).join(sdk.to_string());

        let (bin_dir, extra_paths, env_keys): (Option<String>, Vec<String>, Vec<String>) = {
            let conf = self.config.find_sdk_ok(sdk)?;
            (
                conf.bin_dir.clone(),
                conf.extra_paths.clone(),
                conf.extra_vars.keys().cloned().collect(),
            )
        };

        // 1. 删 symlink（尽力而为，remove_symlink 对不存在路径幂等返回 Ok）
        if let Err(e) = remove_symlink(&sdk_symlink_dir) {
            warning!("failed to remove symlink `{}`: {}", sdk_symlink_dir.display(), e);
        }

        // 2. PATH 清理：主 bin 目录 + extra_paths（尽力而为，remove_sdk_path 跨平台幂等）
        let main_bin = sdk_symlink_dir.join(bin_dir.as_deref().unwrap_or(""));
        if let Err(e) = self.env_operation.remove_sdk_path(&main_bin.to_string_lossy()) {
            warning!("failed to remove main path `{}`: {}", main_bin.display(), e);
        }
        for extra in &extra_paths {
            let extra_bin = sdk_symlink_dir.join(extra);
            if let Err(e) = self.env_operation.remove_sdk_path(&extra_bin.to_string_lossy()) {
                warning!("failed to remove extra path `{}`: {}", extra_bin.display(), e);
            }
        }

        // 3. env 清理：遍历 extra_vars 的 keys 逐个 unset（尽力而为）
        for key in &env_keys {
            if let Err(e) = self.env_operation.unset_sdk_env(key) {
                warning!("failed to unset env `{}`: {}", key, e);
            }
        }

        // 4. config: current_version = None（复用 config 快照回滚，写失败才 bail）
        let snapshot = take_config_snapshot()?;
        {
            let sdk_conf_mut = self.config.find_sdk_mut_ok(sdk)?;
            sdk_conf_mut.current_version = None;
        }
        if let Err(e) = self.config.write_to_disk() {
            warning!("failed to write config, rolling back...");
            let _ = rollback_config(&snapshot);
            bail!("failed to persist config after uninstall: {}", e);
        }
        Ok(())
    }

    /// 删除指定版本目录（不存在视为已删；删失败重试后仍失败则 warning 告知手动处理，返 false 不中断）
    fn remove_version_dir(&self, dir: &Path) -> Result<bool> {
        if !dir.exists() {
            return Ok(true);
        }
        info!("removing version directory: {}", dir.display());
        // Windows 上目录可能被杀毒/索引/进程短暂锁定，重试几次通常可恢复
        let mut last_err: Option<std::io::Error> = None;
        for attempt in 0..3u32 {
            match std::fs::remove_dir_all(dir) {
                Ok(()) => return Ok(true),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
            }
        }
        warning!(
            "failed to remove directory `{}` (may be locked by another process or antivirus)",
            dir.display()
        );
        if let Some(e) = last_err {
            detail!("reason: {}", e);
        }
        detail!("please remove it manually: {}", dir.display());
        Ok(false)
    }

    /// 删完版本后若 store/<sdk> 为空目录则清理（尽力而为，失败仅 warning）
    fn cleanup_empty_sdk_store_dir(&self, sdk: &Sdk) -> Result<()> {
        let sdk_store = get_installed_sdks_dir()?.join(sdk.to_string());
        if sdk_store.exists() {
            let is_empty = std::fs::read_dir(&sdk_store).map(|mut it| it.next().is_none()).unwrap_or(false);
            if is_empty {
                if let Err(e) = std::fs::remove_dir(&sdk_store) {
                    warning!("failed to remove empty store dir `{}`: {}", sdk_store.display(), e);
                }
            }
        }
        Ok(())
    }
}
