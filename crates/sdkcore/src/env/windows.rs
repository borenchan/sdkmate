use std::{env, process::Command};

use crate::env::EnvOperation;
use anyhow::{anyhow, Ok, Result};
use util::{consts::{ENV_JAVA_HOME, ENV_PATH}, info, sdk::Sdk, success, warning};
use windows_sys::Win32::UI::WindowsAndMessaging::HWND_BROADCAST;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::RegKey;

pub struct WindowsEnvOperation;

impl EnvOperation for WindowsEnvOperation {
    fn set_sdk_env(&self, sdk: Sdk, sdk_path: &str) -> Result<()> {
        // Implementation for setting SDK environment variables on Windows
        let env_var_key = match sdk {
            Sdk::Java => ENV_JAVA_HOME,
            _ => return Err(anyhow!("Unsupported Sdk")),
        };
        Command::new("setx")
            .arg(env_var_key)
            .arg(sdk_path)
            .arg("/M") // set system environment variable
            .output()?;
        //set current process env
        unsafe { env::set_var(env_var_key, sdk_path) };
        Ok(())
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()> {
        let key = open_user_env(true)?;
        let current: String = key.get_value(ENV_PATH).unwrap_or_default();

        // 检查是否已存在
        let paths: Vec<&str> = current.split(';').collect();
        if paths.iter().any(|p| p.eq_ignore_ascii_case(sdk_path)) {
            warning!("path exists. sdk_path: {}", sdk_path);
            return Ok(());
        }

        let new_value = if current.is_empty() || current.ends_with(';') {
            format!("{}{}", current, sdk_path)
        } else {
            format!("{};{}", current, sdk_path)
        };

        // 必须用 REG_EXPAND_SZ 类型保存，以支持 %VAR% 语法
        key.set_value(ENV_PATH, &new_value)?;
        broadcast_env_change();
        info!("success add `{sdk_path}` to path!");
        Ok(())
    }

    fn get_path(&self) -> Result<String> {
        let key = open_user_env(false)?;
        // 读取 REG_EXPAND_SZ 原始值，不展开变量
        let path: String = key.get_value(ENV_PATH)?;
        Ok(path)
    }
}

const ENV_KEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";

fn open_user_env(write: bool) -> Result<RegKey> {
    let hkcu = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = if write {
        hkcu.open_subkey_with_flags(ENV_KEY, KEY_READ | KEY_WRITE)?
    } else {
        hkcu.open_subkey(ENV_KEY)?
    };
    std::prelude::rust_2015::Ok(key)
}


pub fn remove_from_path(target: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let key = open_user_env(true)?;
    let current: String = key.get_value(ENV_PATH).unwrap_or_default();

    let new_value: Vec<&str> = current
        .split(';')
        .filter(|p| !p.eq_ignore_ascii_case(target))
        .collect();
    let new_value = new_value.join(";");

    key.set_value(ENV_PATH, &new_value)?;
    broadcast_env_change();
    std::prelude::rust_2015::Ok(())
}

/// 广播环境变量变更，让 Explorer 和其他程序感知
fn broadcast_env_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let msg: Vec<u16> = OsStr::new("Environment\0")
        .encode_wide()
        .collect();

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