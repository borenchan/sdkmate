use crate::env::EnvOperation;
use anyhow::Result;
use std::collections::HashMap;
use util::{consts::ENV_PATH, detail, info, warning};
use windows_sys::Win32::UI::WindowsAndMessaging::HWND_BROADCAST;
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};

pub struct WindowsEnvOperation;

impl EnvOperation for WindowsEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<()> {
        let key = open_env_key(true)?;
        for (env_key, env_val) in envs {
            key.set_value(env_key, env_val)?;
            info!("success set env key:`{env_key}` value:`{env_val}` !");
        }
        broadcast_env_change();
        Ok(())
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()> {
        let key = open_env_key(true)?;
        let current: String = key.get_value(ENV_PATH).unwrap_or_default();

        // 检查是否已存在（大小写不敏感）
        if current.split(';').any(|p| p.eq_ignore_ascii_case(sdk_path)) {
            warning!("path exists. sdk_path: {}", sdk_path);
            return Ok(());
        }

        // 前置添加 sdkm 路径，确保优先级最高
        let new_value = format!("{};{}", sdk_path, current);
        key.set_value(ENV_PATH, &new_value)?;
        broadcast_env_change();
        info!("success add `{sdk_path}` to path!");
        Ok(())
    }

    fn get_path(&self) -> Result<String> {
        let key = open_env_key(false)?;
        let path: String = key.get_value(ENV_PATH)?;
        Ok(path)
    }

    fn remove_sdk_path(&self, target: &str) -> Result<()> {
        let key = open_env_key(true)?;
        let current: String = key.get_value(ENV_PATH).unwrap_or_default();

        let new_value: String = current
            .split(';')
            .filter(|p| !p.eq_ignore_ascii_case(target))
            .collect::<Vec<&str>>()
            .join(";");

        // PATH 中不存在该条目则幂等返回（不重复写注册表、不打印误导性 removed）
        if new_value == current {
            return Ok(());
        }
        key.set_value(ENV_PATH, &new_value)?;
        broadcast_env_change();
        detail!("removed `{target}` from PATH");
        Ok(())
    }

    fn get_env_value(&self, key: &str) -> Result<Option<String>> {
        let hklm_key = open_env_key(false)?;
        let val: std::result::Result<String, _> = hklm_key.get_value(key);
        match val {
            std::result::Result::Ok(v) => Ok(Some(v)),
            std::result::Result::Err(_) => Ok(None), // 注册表中不存在该值
        }
    }

    fn unset_sdk_env(&self, key: &str) -> Result<()> {
        let reg_key = open_env_key(true)?;
        // 值不存在时忽略错误（可能已被手动删除）
        let result: std::result::Result<(), _> = reg_key.delete_value(key);
        if result.is_ok() {
            detail!("removed env `{key}`");
        } else {
            warning!("env `{key}` does not exist, skip unset");
        }
        broadcast_env_change();
        Ok(())
    }

    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<()> {
        let reg_key = open_env_key(true)?;
        for (env_key, old_val) in old_envs {
            if let Some(val) = old_val {
                // 有旧值，写回注册表
                reg_key.set_value(env_key, val)?;
                info!("restored env `{env_key}` to `{val}`");
            } else {
                // 之前不存在，删除当前值
                self.unset_sdk_env(env_key)?;
            }
        }
        broadcast_env_change();
        Ok(())
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
            std::ptr::null_mut(),
        );
    }
}
