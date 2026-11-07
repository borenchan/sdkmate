use crate::env::{EnvAction, EnvOperation};
use anyhow::Result;
use std::collections::HashMap;
use util::consts::ENV_PATH;
use windows_sys::Win32::UI::WindowsAndMessaging::HWND_BROADCAST;
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};

pub struct WindowsEnvOperation;

impl EnvOperation for WindowsEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<Vec<EnvAction>> {
        let key = open_env_key(true)?;
        let mut actions = Vec::with_capacity(envs.len());
        for (env_key, env_val) in envs {
            key.set_value(env_key, env_val)?;
            actions.push(EnvAction::EnvSet {
                key: env_key.clone(),
                value: env_val.clone(),
            });
        }
        broadcast_env_change();
        Ok(actions)
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<Vec<EnvAction>> {
        let key = open_env_key(true)?;
        let current: String = key.get_value(ENV_PATH).unwrap_or_default();

        // 检查是否已存在（大小写不敏感）
        if current.split(';').any(|p| p.eq_ignore_ascii_case(sdk_path)) {
            return Ok(vec![EnvAction::PathExists {
                path: sdk_path.to_string(),
            }]);
        }

        // 前置添加 sdkm 路径，确保优先级最高
        let new_value = format!("{};{}", sdk_path, current);
        key.set_value(ENV_PATH, &new_value)?;
        broadcast_env_change();
        Ok(vec![EnvAction::PathAdded {
            path: sdk_path.to_string(),
        }])
    }

    fn get_path(&self) -> Result<String> {
        let key = open_env_key(false)?;
        let path: String = key.get_value(ENV_PATH)?;
        Ok(path)
    }

    fn remove_sdk_path(&self, target: &str) -> Result<Vec<EnvAction>> {
        let key = open_env_key(true)?;
        let current: String = key.get_value(ENV_PATH).unwrap_or_default();

        let new_value: String = current
            .split(';')
            .filter(|p| !p.eq_ignore_ascii_case(target))
            .collect::<Vec<&str>>()
            .join(";");

        // PATH 中不存在该条目则幂等返回（不重复写注册表、不发误导性 removed）
        if new_value == current {
            return Ok(Vec::new());
        }
        key.set_value(ENV_PATH, &new_value)?;
        broadcast_env_change();
        Ok(vec![EnvAction::PathRemoved {
            path: target.to_string(),
        }])
    }

    fn get_env_value(&self, key: &str) -> Result<Option<String>> {
        let hklm_key = open_env_key(false)?;
        match hklm_key.get_value(key) {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None), // 注册表中不存在该值
        }
    }

    fn unset_sdk_env(&self, key: &str) -> Result<Vec<EnvAction>> {
        let reg_key = open_env_key(true)?;
        // 值不存在时忽略错误（可能已被手动删除）
        let result = reg_key.delete_value(key);
        broadcast_env_change();
        match result {
            Ok(()) => Ok(vec![EnvAction::EnvRemoved { key: key.to_string() }]),
            Err(_) => Ok(vec![EnvAction::EnvMissing { key: key.to_string() }]),
        }
    }

    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<Vec<EnvAction>> {
        let reg_key = open_env_key(true)?;
        let mut actions = Vec::with_capacity(old_envs.len());
        for (env_key, old_val) in old_envs {
            if let Some(val) = old_val {
                // 有旧值，写回注册表
                reg_key.set_value(env_key, val)?;
                actions.push(EnvAction::EnvRestored {
                    key: env_key.clone(),
                    value: val.clone(),
                });
            } else {
                // 之前不存在，删除当前值
                actions.extend(self.unset_sdk_env(env_key)?);
            }
        }
        broadcast_env_change();
        Ok(actions)
    }
}

// HKLM 的系统 Environment：系统级环境变量，需管理员权限
const ENV_KEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";

fn open_env_key(write: bool) -> Result<RegKey> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = if write {
        hklm.open_subkey_with_flags(ENV_KEY, KEY_READ | KEY_WRITE)?
    } else {
        hklm.open_subkey(ENV_KEY)?
    };
    Ok(key)
}

/// 广播环境变量变更，让 Explorer 和其他程序感知
fn broadcast_env_change() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE};

    let msg: Vec<u16> = OsStr::new("Environment\0").encode_wide().collect();

    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            msg.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }
}
