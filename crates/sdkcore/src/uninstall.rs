// 卸载本地已安装的 SDK 版本
//
// 复用既有逻辑（跨模块调用的均为已 pub 的公共 API，无需抽取通用文件）：
// - list_local_sdk_versions / fuzzy_match_version_core：版本扫描（静默）+ 模糊匹配（与 install/switch 同源）
// - switch_sdk_to_version：卸载当前版本且有其他版本时，复用 switch 流程切到其他版本（业务编排）
// - remove_symlink / EnvOperation trait 方法 / config 快照回滚：底层公共 API

use crate::config::{rollback_config, take_config_snapshot};
use crate::env::report_env_actions;
use crate::link::symlink::remove_symlink;
use crate::manager::SdkManager;
use crate::project_config::find_project_config;
use crate::version::fuzzy_match_version_core;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use util::path::get_installed_sdks_dir;
use util::sdk::Sdk;

/// resolve_uninstall_target 的返回：目标版本 + 模糊命中 + active 状态与卸载后将切换到的版本
/// （调用端据此组装确认文案；文案说什么归 core 数据，怎么展示归前端）
pub struct ResolvedUninstall {
    pub version: String,
    pub fuzzy_matched: bool,
    pub is_active: bool,
    /// active 时：卸载后将切换到的其他已装版本（第一个，非"最新"）
    pub other_version: Option<String>,
}

impl SdkManager {
    /// 卸载目标预检（只读）：解析目标版本 + active/其他版本状态，供调用端组装确认文案；
    /// 模糊与破坏性确认在调用端做（core 零 stdin）。执行段自行重算这些状态，不信任本结果
    pub fn resolve_uninstall_target(&self, sdk: &Sdk, version_input: &str) -> Result<ResolvedUninstall> {
        let versions = self.list_local_sdk_versions(sdk)?;
        if versions.is_empty() {
            bail!("No installed versions for `{}`. Nothing to uninstall.", sdk);
        }

        // 模糊匹配本地已装版本（复用 fuzzy 核心，找不到匹配内部带 did-you-mean）
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = fuzzy_match_version_core(&version_strings, version_input)?;
        let target_version = matched.full_version;

        let target_item = versions.iter().find(|v| v.sdk_version == target_version).context(format!(
            "local not found `{}` version `{}`, please check store dir",
            sdk, target_version
        ))?;
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

        Ok(ResolvedUninstall {
            version: target_version,
            fuzzy_matched: matched.fuzzy_matched,
            is_active,
            other_version,
        })
    }

    /// 卸载本地已安装的 SDK 版本
    ///
    /// - 非当前激活版本：仅删 `store/<sdk>/<version>` 目录
    /// - 当前激活版本且有其他已装版本：先 switch 到其他版本再删（复用 switch 流程）
    /// - 当前激活版本且仅此一版：清理 symlink/PATH/env/current 后再删
    ///
    /// 破坏性确认归调用端（resolve_uninstall_target + 调用端确认后传入精确版本）
    pub fn uninstall_sdk(&mut self, sdk: &Sdk, version_input: &str) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        if versions.is_empty() {
            bail!("No installed versions for `{}`. Nothing to uninstall.", sdk);
        }

        // 模糊匹配本地已装版本（复用 fuzzy 核心，找不到匹配内部带 did-you-mean）——
        // 执行段自查：调用端确认期间状态可能变化，不信任预检查结果
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

        // active 时是否有其他已装版本（取第一个，不做"最新"计算）
        let other_version: Option<String> = if is_active {
            versions
                .iter()
                .find(|v| v.sdk_version != target_version)
                .map(|v| v.sdk_version.clone())
        } else {
            None
        };

        // ── 卸载前只读检测（只提示不阻断）──
        if is_active {
            self.sink.warning(format!(
                "You are uninstalling the globally active version of `{sdk}`. \
                 IDEs/non-shell tools using the global symlink may break until you switch."
            ));
        }
        for path in Self::check_project_references(sdk, &target_version) {
            self.sink.warning(format!(
                "Project config '{}' pins {} = {}. After uninstall, `sdkm env` will fall back \
                 to global/system for that project.",
                path.display(),
                sdk,
                target_version
            ));
        }

        self.sink.info(format!("uninstalling `{sdk}` `{target_version}`..."));

        if is_active {
            match &other_version {
                Some(other) => {
                    self.sink.info(format!("switching to {} {} before uninstall...", sdk, other));
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
            self.sink
                .success(format!("uninstall `{sdk}` version `{target_version}` success!"));
        } else {
            self.sink.warning(format!(
                "uninstall `{sdk}` `{target_version}` partially complete: version directory remains at `{}` (may be locked by another process or antivirus), please remove it manually",
                target_sdk_dir.display()
            ));
        }
        Ok(())
    }

    /// 清除 SDK 的全部激活环境（卸载当前唯一版本时调用）
    ///
    /// 尽力而为：symlink/PATH/env 清理失败仅 warning 不中断后续步骤；
    /// 仅 config（current_version=None）写入失败时回滚并 bail（避免残留指向已删版本的幽灵 current）。
    pub(crate) fn cleanup_sdk_environment(&mut self, sdk: &Sdk) -> Result<()> {
        self.sink
            .info(format!("cleaning up `{sdk}` environment (symlink/PATH/env/current_version)..."));
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

        // 1. 删 symlink（尽力而为，remove_symlink 对不存在路径幂等返回 Ok；实删才报 removed）
        match remove_symlink(&sdk_symlink_dir) {
            Err(e) => {
                self.sink
                    .warning(format!("failed to remove symlink `{}`: {}", sdk_symlink_dir.display(), e));
            }
            Ok(true) => {
                self.sink.detail(format!("removed symlink at {}", sdk_symlink_dir.display()));
            }
            Ok(false) => {}
        }

        // 2. PATH 清理：主 bin 目录 + extra_paths（尽力而为，remove_sdk_path 跨平台幂等）
        let main_bin = sdk_symlink_dir.join(bin_dir.as_deref().unwrap_or(""));
        match self.env_operation.remove_sdk_path(&main_bin.to_string_lossy()) {
            Err(e) => {
                self.sink
                    .warning(format!("failed to remove main path `{}`: {}", main_bin.display(), e));
            }
            Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
        }
        for extra in &extra_paths {
            let extra_bin = sdk_symlink_dir.join(extra);
            match self.env_operation.remove_sdk_path(&extra_bin.to_string_lossy()) {
                Err(e) => {
                    self.sink
                        .warning(format!("failed to remove extra path `{}`: {}", extra_bin.display(), e));
                }
                Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
            }
        }

        // 3. env 清理：遍历 extra_vars 的 keys 逐个 unset（尽力而为）
        for key in &env_keys {
            match self.env_operation.unset_sdk_env(key) {
                Err(e) => {
                    self.sink.warning(format!("failed to unset env `{key}`: {e}"));
                }
                Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
            }
        }

        // 4. config: current_version = None（复用 config 快照回滚，写失败才 bail）
        let snapshot = take_config_snapshot()?;
        {
            let sdk_conf_mut = self.config.find_sdk_mut_ok(sdk)?;
            sdk_conf_mut.current_version = None;
        }
        if let Err(e) = self.config.write_to_disk() {
            self.sink.warning("failed to write config, rolling back...".to_string());
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
        self.sink.info(format!("removing version directory: {}", dir.display()));
        // Windows 上目录可能被杀毒/索引/进程短暂锁定，重试几次通常可恢复
        let mut last_err: Option<io::Error> = None;
        for attempt in 0..3u32 {
            match fs::remove_dir_all(dir) {
                Ok(()) => return Ok(true),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        thread::sleep(Duration::from_millis(300));
                    }
                }
            }
        }
        self.sink.warning(format!(
            "failed to remove directory `{}` (may be locked by another process or antivirus)",
            dir.display()
        ));
        if let Some(e) = last_err {
            self.sink.detail(format!("reason: {e}"));
        }
        self.sink.detail(format!("please remove it manually: {}", dir.display()));
        Ok(false)
    }

    /// 删完版本后若 store/<sdk> 为空目录则清理（尽力而为，失败仅 warning）
    fn cleanup_empty_sdk_store_dir(&self, sdk: &Sdk) -> Result<()> {
        let sdk_store = get_installed_sdks_dir()?.join(sdk.to_string());
        if sdk_store.exists() {
            let is_empty = fs::read_dir(&sdk_store).map(|mut it| it.next().is_none()).unwrap_or(false);
            if is_empty && let Err(e) = fs::remove_dir(&sdk_store) {
                self.sink
                    .warning(format!("failed to remove empty store dir `{}`: {}", sdk_store.display(), e));
            }
        }
        Ok(())
    }

    /// 检测当前目录向上的 `.sdkm.toml` 是否引用该 SDK+版本（只读，只提示不阻断）
    ///
    /// 返回引用了该版本的配置文件路径列表。
    fn check_project_references(sdk: &Sdk, version: &str) -> Vec<PathBuf> {
        let Some((path, project_cfg)) = find_project_config().ok().flatten() else {
            return Vec::new();
        };
        // pin 可能是模糊值（如 "21"：用户手写 .sdkm.toml），不能精确比较——
        // 把被卸载版本当候选池、pin 当输入做模糊匹配，命中即视为引用
        match project_cfg.pins.get(&sdk.to_string()) {
            Some(pin) if fuzzy_match_version_core(&[version.to_string()], pin).is_ok() => vec![path],
            _ => Vec::new(),
        }
    }
}
