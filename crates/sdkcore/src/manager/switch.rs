use crate::env::split_path_entries;
use crate::link::symlink::create_symlink;
use crate::manager::SdkManager;
use crate::manager::config::SdkConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use util::config_helper::PLACEHOLDER_SDK_DIR;
use util::sdk::Sdk;
use util::success;
use util::{info, warning};

impl SdkManager {
    pub fn switch_sdk_to_version(&mut self, sdk: &Sdk, sdk_version: &str) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let sdk_conf = self.config.find_sdk_ok(&sdk)?;
        let is_active = versions.iter().any(|v| v.is_active && v.sdk_version == sdk_version);
        if !is_active {
            let current_version_sdk = versions
                .into_iter()
                .find(|v| v.sdk_version == sdk_version)
                .context(format!("not found `{sdk}` version `{sdk_version}`, please check sdk's dir!"))?;
            let symlink_root_dir = self.config.symlink_dir.clone();
            let sdk_symlink_dir = PathBuf::from(symlink_root_dir).join(sdk.to_string());
            create_symlink(&current_version_sdk.sdk_dir, &sdk_symlink_dir)?;
            let sdk_symlink_bin_dir = sdk_symlink_dir.join(sdk_conf.bin_dir.as_str());
            // add sdk symlink link to current active version dir
            let sdk_symlink_bin_cow = sdk_symlink_bin_dir.to_string_lossy();
            let path = self.env_operation.get_path()?;

            // ── PATH 冲突检测：检测非 sdkm 来源的同名 SDK 路径 ──
            let conflicts = self.detect_path_conflicts(sdk, &path)?;
            if !conflicts.is_empty() {
                self.handle_path_conflicts(&conflicts)?;
            }

            // add sdk path only when does not exist in the os path
            if !path.contains(sdk_symlink_bin_cow.as_ref()) {
                self.env_operation
                    .set_sdk_envs(&Self::get_sdk_extra_envs(sdk_conf, &sdk_symlink_dir)?)?;
                self.env_operation.add_sdk_path(sdk_symlink_bin_cow.as_ref())?;
            }

            // ── extra_paths：为每个额外路径添加到 PATH ──
            for extra_path in &sdk_conf.extra_paths {
                let extra_bin_dir = sdk_symlink_dir.join(extra_path);
                if extra_bin_dir.exists() {
                    let extra_bin_cow = extra_bin_dir.to_string_lossy();
                    if !path.contains(extra_bin_cow.as_ref()) {
                        self.env_operation.add_sdk_path(extra_bin_cow.as_ref())?;
                    }
                } else {
                    warning!("extra_path `{}` does not exist, skipping", extra_path);
                }
            }

            //todo error restore link and path
            //success switch version, need update config
            let sdk_conf = self.config.find_sdk_mut_ok(&sdk)?;
            sdk_conf.current_version = Some(sdk_version.to_string());
            self.config.write_to_disk()?;
            // ── 终端重启提示：PATH 修改后当前终端不会立刻生效 ──
            info!("PATH has been updated. Please restart your terminal for changes to take effect.");
        }
        success!("switch sdk `{}` to version `{}` success!", sdk, sdk_version);
        Ok(())
    }

    fn get_sdk_extra_envs(sdk_conf: &SdkConfig, sdk_symlink_dir: &Path) -> Result<HashMap<String, String>> {
        let mut env = HashMap::with_capacity(1);
        let sdk_dir = sdk_symlink_dir.to_string_lossy();
        env.insert(PLACEHOLDER_SDK_DIR, sdk_dir.as_ref());
        let actual_extra_vars = sdk_conf.get_actual_extra_vars(&env)?;
        Ok(actual_extra_vars)
    }

    /// 检测 PATH 中非 sdkm 来源的同名 SDK 路径（仅对内置 SDK）
    fn detect_path_conflicts(&self, sdk: &Sdk, path: &str) -> Result<Vec<String>> {
        let builtin = match sdk {
            Sdk::Built(b) => b,
            Sdk::Custom(_) => return Ok(Vec::new()),
        };

        let symlink_root = &self.config.symlink_dir;
        let entries = split_path_entries(path);
        let executables = builtin.primary_executables();

        // Windows 检查 .exe 和 .cmd；Unix 检查原始名
        let extensions: &[&str] = if cfg!(windows) { &[".exe", ".cmd"] } else { &[""] };

        let mut conflicts = Vec::new();
        for entry in &entries {
            // 跳过 sdkm 管理的路径
            if entry.starts_with(symlink_root) {
                continue;
            }

            let entry_path = PathBuf::from(entry);
            if !entry_path.is_dir() {
                continue;
            }

            // 检查该目录是否包含 SDK 的主可执行文件
            let found = executables
                .iter()
                .any(|exe| extensions.iter().any(|ext| entry_path.join(format!("{}{}", exe, ext)).exists()));
            if found {
                conflicts.push(entry.clone());
            }
        }

        Ok(conflicts)
    }

    /// 处理 PATH 冲突：仅警告，不移除任何 PATH 条目
    /// sdkm 路径前置添加（最高优先级），冲突路径自然被覆盖
    fn handle_path_conflicts(&self, conflicts: &[String]) -> Result<()> {
        for conflict in conflicts {
            warning!("Found existing SDK path at '{}' in PATH.", conflict);
        }
        info!("sdkm's path has highest priority, these conflicts won't affect your usage.");
        Ok(())
    }
}
