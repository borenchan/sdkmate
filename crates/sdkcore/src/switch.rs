use crate::config::SdkConfig;
use crate::config::SdkmConfig;
use crate::env::split_path_entries;
use crate::link::symlink::{create_symlink, read_symlink_target, remove_symlink};
use crate::manager::SdkManager;
use crate::version::fuzzy_match_version_core;
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use util::config_helper::PLACEHOLDER_SDK_DIR;
use util::consts::BugReportError;
use util::sdk::Sdk;
use util::success;
use util::terminal::prompt_confirm;
use util::{detail, info, warning};

/// switch 操作前的系统状态快照，用于失败时回滚
struct SwitchSnapshot {
    /// 符号链接路径（如 C:\sdkm\symlink\java）
    symlink_path: PathBuf,
    /// 旧链接指向的目标路径，None 表示旧链接不存在
    old_symlink_target: Option<PathBuf>,
    /// 环境变量旧值，None 表示变量之前不存在
    old_env_values: HashMap<String, Option<String>>,
    /// 本次 switch 添加到 PATH 的所有条目（回滚时按反序 remove）
    added_path_entries: Vec<String>,
    /// 内存级 config 快照（SdkmConfig 已 derive Clone）
    old_config: SdkmConfig,
}

/// 回滚不可恢复的项目：记录对用户的影响描述和修复建议
struct RollbackFailure {
    /// 对用户的影响描述
    description: String,
    /// 建议用户采取的下一步行动
    suggestion: &'static str,
}

// ── try_step! 宏：执行操作，失败时自动回滚并提前返回 ──
//
// 消除 Phase 3 中 5 处重复的 `if-let-Err + rollback + return Err(e)` 模式。
// 已知权限错误（access denied / privilege not held）不标记，未知错误标记 BugReportError。
macro_rules! try_step {
    ($expr:expr, $snapshot:expr, $manager:expr, $msg:expr $(, $fmt_arg:expr)* $(,)?) => {
        match $expr {
            Err(e) => {
                warning!($msg $(, $fmt_arg)*);
                rollback($snapshot, $manager)?;
                // 已知权限错误（access denied 5 / EACCES 13、privilege not held 1314）属用户环境问题，
                // 提示自行解决；其余未知错误标记 bug report
                let is_perm = e.downcast_ref::<io::Error>()
                    .and_then(|ie| ie.raw_os_error())
                    .map_or(false, |code| matches!(code, 5 | 13 | 1314));
                if is_perm {
                    let hint = if cfg!(windows) { "run sdkm as administrator" } else { "check permissions" };
                    bail!("{}: {hint}", e);
                }
                return Err(BugReportError::wrap(e));
            }
            Ok(val) => val,
        }
    };
}

impl SdkManager {
    pub fn switch_sdk_to_version(&mut self, sdk: &Sdk, sdk_version: &str) -> Result<()> {
        // ── Phase 0: 前置检查(只读操作,无副作用) ──
        let versions = self.list_local_sdk_versions(sdk)?;
        let sdk_conf = self.config.find_sdk_ok(sdk)?;

        // 模糊匹配本地已安装版本(与 install 共用核心)
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = fuzzy_match_version_core(&version_strings, sdk_version)?;
        let target_version = matched.full_version;

        // 模糊命中时交互确认(与 install 一致)
        if matched.fuzzy_matched {
            let confirmed = prompt_confirm(&format!(
                "Resolved '{}' → '{}'. Switch to this version?",
                sdk_version, target_version
            ))?;
            if !confirmed {
                bail!("Switch cancelled by user");
            }
        }

        let is_active = versions.iter().any(|v| v.is_active && v.sdk_version == target_version);
        if is_active {
            success!("switch sdk `{}` to version `{}` success!", sdk, target_version);
            return Ok(());
        }
        let current_version_sdk = versions.into_iter().find(|v| v.sdk_version == target_version).context(format!(
            "local not found `{sdk}` version `{target_version}`, please check store dir or install the version!"
        ))?;

        let symlink_root_dir = self.config.resolved_symlink_dir()?;
        let sdk_symlink_dir = PathBuf::from(symlink_root_dir).join(sdk.to_string());
        let sdk_symlink_bin_dir = sdk_symlink_dir.join(sdk_conf.bin_dir.as_deref().unwrap_or(""));
        let sdk_symlink_bin_cow = sdk_symlink_bin_dir.to_string_lossy();
        let path = self.env_operation.get_path()?;

        // ── Phase 1: PATH 冲突检测（只读操作） ──
        let conflicts = self.detect_path_conflicts(sdk, &path)?;
        if !conflicts.is_empty() {
            self.handle_path_conflicts(sdk, &conflicts)?;
        }

        // ── Phase 2: 备份旧状态 ──
        let old_symlink_target = read_symlink_target(&sdk_symlink_dir)?;
        let extra_envs = Self::get_sdk_extra_envs(sdk_conf, &sdk_symlink_dir)?;
        let old_env_values: HashMap<String, Option<String>> = extra_envs
            .keys()
            .map(|k| (k.clone(), self.env_operation.get_env_value(k).unwrap_or(None)))
            .collect();
        let old_config = self.config.clone();

        let mut snapshot = SwitchSnapshot {
            symlink_path: sdk_symlink_dir.clone(),
            old_symlink_target,
            old_env_values,
            added_path_entries: Vec::new(),
            old_config,
        };

        // ── Phase 3: 逐步执行修改，失败时回滚 ──

        // Step 3a: 创建符号链接
        try_step!(
            create_symlink(&current_version_sdk.sdk_dir, &sdk_symlink_dir),
            &snapshot,
            self,
            "Failed to create symlink, rolling back..."
        );

        // 仅当 bin 目录不在 PATH 中时才添加环境变量和路径
        let need_add_path = !path.contains(sdk_symlink_bin_cow.as_ref());

        // Step 3b: 设置额外环境变量
        if need_add_path {
            try_step!(
                self.env_operation.set_sdk_envs(&extra_envs),
                &snapshot,
                self,
                "Failed to set env vars, rolling back..."
            );
        }

        // Step 3c: 添加主 bin 目录到 PATH
        if need_add_path {
            try_step!(
                self.env_operation.add_sdk_path(sdk_symlink_bin_cow.as_ref()),
                &snapshot,
                self,
                "Failed to add main path, rolling back..."
            );
            snapshot.added_path_entries.push(sdk_symlink_bin_cow.to_string());
        }

        // Step 3d: 处理 extra_paths
        for extra_path in &sdk_conf.extra_paths {
            let extra_bin_dir = sdk_symlink_dir.join(extra_path);
            if extra_bin_dir.exists() {
                let extra_bin_cow = extra_bin_dir.to_string_lossy();
                if !path.contains(extra_bin_cow.as_ref()) {
                    try_step!(
                        self.env_operation.add_sdk_path(extra_bin_cow.as_ref()),
                        &snapshot,
                        self,
                        "Failed to add extra path `{}`, rolling back...",
                        extra_path
                    );
                    snapshot.added_path_entries.push(extra_bin_cow.to_string());
                }
            } else {
                warning!("extra_path `{}` does not exist, skipping", extra_path);
            }
        }

        // Step 3e: 更新 config 并写磁盘
        {
            let sdk_conf_mut = self.config.find_sdk_mut_ok(sdk)?;
            sdk_conf_mut.current_version = Some(target_version.clone());
        }
        try_step!(
            self.config.write_to_disk(),
            &snapshot,
            self,
            "Failed to write config, rolling back..."
        );

        // ── Phase 4: 成功完成 ──
        // hook 已注入时按回车（prompt 触发 sdkm env 重建 PATH）即生效；未注入（手动 eval 场景）仍需重启
        info!("PATH has been updated. Press Enter to apply, or restart your terminal if hooks are not installed.");
        success!("switch sdk `{}` to version `{}` success!", sdk, target_version);
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

        let symlink_root = self.config.resolved_symlink_dir()?;
        let entries = split_path_entries(path);
        let executables = builtin.primary_executables();

        // Windows 检查 .exe 和 .cmd；Unix 检查原始名
        let extensions: &[&str] = if cfg!(windows) { &[".exe", ".cmd"] } else { &[""] };

        let mut conflicts = Vec::new();
        for entry in &entries {
            // 跳过 sdkm 管理的路径
            if entry.starts_with(&symlink_root) {
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
    fn handle_path_conflicts(&self, sdk: &Sdk, conflicts: &[String]) -> Result<()> {
        for conflict in conflicts {
            warning!("Found existing SDK[{}] path at '{}' in PATH.", sdk, conflict);
        }
        info!("sdkm's path has highest priority, these conflicts won't affect your usage.");
        Ok(())
    }
}

// ── 回滚函数 ──

/// 回滚符号链接：删除新链接，恢复旧链接（如果旧链接存在）
fn rollback_symlink(snapshot: &SwitchSnapshot) -> Result<()> {
    remove_symlink(&snapshot.symlink_path)?;
    if let Some(old_target) = &snapshot.old_symlink_target {
        create_symlink(old_target, &snapshot.symlink_path)?;
    }
    Ok(())
}

/// 回滚环境变量：有旧值写回，无旧值删除
fn rollback_envs(snapshot: &SwitchSnapshot, manager: &SdkManager) -> Result<()> {
    let mut restore_envs = HashMap::new();
    let mut unset_keys = Vec::new();
    for (key, old_val) in &snapshot.old_env_values {
        if old_val.is_some() {
            restore_envs.insert(key.clone(), old_val.clone());
        } else {
            unset_keys.push(key.clone());
        }
    }
    if !restore_envs.is_empty() {
        manager.env_operation.restore_sdk_envs(&restore_envs)?;
    }
    for key in unset_keys {
        manager.env_operation.unset_sdk_env(&key)?;
    }
    Ok(())
}

/// 回滚 PATH：按反序移除所有已添加的路径条目
fn rollback_paths(snapshot: &SwitchSnapshot, manager: &SdkManager) -> Result<()> {
    for path_entry in snapshot.added_path_entries.iter().rev() {
        manager.env_operation.remove_sdk_path(path_entry)?;
    }
    Ok(())
}

/// 回滚 config：恢复内存级 config 快照
fn rollback_config(snapshot: &SwitchSnapshot, manager: &mut SdkManager) -> Result<()> {
    manager.config = snapshot.old_config.clone();
    Ok(())
}

/// 全量回滚（尽力而为策略：每个步骤失败不中断后续回滚）
/// 只输出对用户有影响的结果，不暴露内部细节
fn rollback(snapshot: &SwitchSnapshot, manager: &mut SdkManager) -> Result<()> {
    let mut failures: Vec<RollbackFailure> = Vec::new();

    // 按反向顺序回滚：config → paths → envs → symlink
    if rollback_config(snapshot, manager).is_err() {
        failures.push(RollbackFailure {
            description: "Config file may be inconsistent with actual state.".into(),
            suggestion: "Run `sdkm switch <sdk> <version>` again to auto-fix.",
        });
    }
    if rollback_paths(snapshot, manager).is_err() {
        failures.push(RollbackFailure {
            description: format!("PATH has stale entries for {}.", snapshot.symlink_path.display()),
            suggestion: "Restart your terminal or run `sdkm switch` again.",
        });
    }
    if rollback_envs(snapshot, manager).is_err() {
        failures.push(RollbackFailure {
            description: "Environment variables (e.g. JAVA_HOME) may point to wrong path.".into(),
            suggestion: "Check your system environment variables settings.",
        });
    }
    if rollback_symlink(snapshot).is_err() {
        failures.push(RollbackFailure {
            description: format!(
                "Symlink at {} could not be restored — SDK may be unavailable.",
                snapshot.symlink_path.display()
            ),
            suggestion: "Try `sdkm switch <sdk> <version>` again or check the symlink manually.",
        });
    }

    if !failures.is_empty() {
        warning!("Switch failed and rollback was incomplete:");
        for f in &failures {
            detail!("• {}", f.description);
            detail!("  → {}", f.suggestion);
        }
    }

    Ok(())
}
