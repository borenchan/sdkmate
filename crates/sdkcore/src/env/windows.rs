use std::{env, process::Command};

use crate::env::EnvOperation;
use anyhow::{Ok, Result, anyhow};
use util::{
    consts::{ENV_JAVA_HOME, ENV_PATH},
    sdk::Sdk,
};
use util::terminal::success;

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
        // Implementation for adding SDK path to PATH environment variable on Windows
        let path_val = env::var(ENV_PATH).unwrap_or_default();
        println!("[add sdk path], path:{}\n,sdk_path:{}", path_val, sdk_path);
        if path_val.contains(sdk_path) {
            return Ok(());
        }

        let new_path_val = if path_val.is_empty() {
            sdk_path.to_string()
        } else {
            format!("{};{}", path_val, sdk_path)
        };
        println!("[add sdk path], new path:{}", new_path_val);
        let output = Command::new("setx")
            .arg(ENV_PATH)
            .arg(new_path_val.as_str())
            .arg("/M") // set system environment variable
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("fail set path"));
        }
        unsafe { env::set_var(ENV_PATH, new_path_val.as_str()) };
        Ok(())
    }
}
