use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use util::{info, warning};
use util::consts::ENV_PATH;

use crate::env::EnvOperation;

pub struct UnixEnvOperation {}

impl UnixEnvOperation {
    fn get_shell_profile_path() -> Result<PathBuf> {
        let home = env::var_os("HOME")
            .context("HOME environment variable not set")?;
        let home = PathBuf::from(home);

        let shell = env::var("SHELL").unwrap_or_default();
        let profile_name = if shell.contains("zsh") {
            ".zshrc"
        } else {
            ".bashrc"
        };

        Ok(home.join(profile_name))
    }

    fn expand_path(path: &str) -> String {
        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                return path.replacen('~', &home, 1);
            }
        }
        path.to_string()
    }

    fn append_or_replaceexport(file_path: &PathBuf, key: &str, value: &str) -> Result<()> {
        let expanded_value = Self::expand_path(value);
        let new_export_line = format!("export {}=\"{}\"", key, expanded_value);

        let content = if file_path.exists() {
            fs::read_to_string(file_path)?
        } else {
            String::new()
        };

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        // Check if export line exists, if so, replace it
        let export_pattern = format!("export {}=", key);
        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with(&export_pattern) {
                *line = new_export_line.clone();
                found = true;
                break;
            }
        }

        // If not found, append
        if !found {
            lines.push(new_export_line);
        }

        let new_content = lines.join("\n");
        fs::write(file_path, new_content)?;

        Ok(())
    }

    fn add_path_entry(file_path: &PathBuf, new_path: &str) -> Result<()> {
        let expanded_path = Self::expand_path(new_path);

        let content = if file_path.exists() {
            fs::read_to_string(file_path)?
        } else {
            String::new()
        };

        // Check if already exists in PATH export
        let path_pattern = format!("export PATH=\"{}", expanded_path);
        if content.contains(&path_pattern) || content.contains(&format!("export PATH={}", expanded_path)) {
            warning!("path exists. sdk_path: {}", new_path);
            return Ok(());
        }

        // Check current PATH from file content
        let current_path = Self::get_path_from_content(&content)?;
        let paths: Vec<&str> = current_path.split(':').collect();
        if paths.iter().any(|p| p == expanded_path) {
            warning!("path exists. sdk_path: {}", new_path);
            return Ok(());
        }

        // Add to PATH export line if exists, otherwise create new one
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let path_export_pattern = "export PATH=";

        let mut found_path_export = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with(path_export_pattern) {
                // Append new path at the beginning
                let current_value = line.trim_start_matches(path_export_pattern).trim_matches('"');
                *line = format!("export PATH=\"{}\":{}", expanded_path, current_value);
                found_path_export = true;
                break;
            }
        }

        if !found_path_export {
            lines.push(format!("export PATH=\"{}\"", expanded_path));
        }

        let new_content = lines.join("\n");
        fs::write(file_path, new_content)?;

        Ok(())
    }

    fn get_path_from_content(content: &str) -> Result<String> {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("export PATH=") {
                let value = line.trim_start_matches("export PATH=").trim_matches('"');
                return Ok(value.to_string());
            }
        }
        Ok(env::var(ENV_PATH).unwrap_or_default())
    }

    fn source_profile(profile_path: &PathBuf) -> Result<()> {
        let shell = env::var("SHELL").unwrap_or_default();
        let profile_name = profile_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(".bashrc");

        // Try to source by executing a new shell
        let output = if shell.contains("zsh") {
            Command::new("zsh")
                .args(["-c", &format!("source '{}' 2>/dev/null; echo $PATH", profile_path.display())])
                .output()?
        } else {
            Command::new("bash")
                .args(["-c", &format!("source '{}' 2>/dev/null; echo $PATH", profile_path.display())])
                .output()?
        };

        if output.status.success() {
            let new_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !new_path.is_empty() {
                env::set_var(ENV_PATH, new_path);
            }
        }

        Ok(())
    }
}

impl EnvOperation for UnixEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;

        for (env_key, env_val) in envs {
            Self::append_or_replaceexport(&profile_path, env_key, env_val)?;
            info!("success set env key:`{}` value:`{}` !", env_key, env_val);
        }

        // Source the profile to apply changes to current process
        Self::source_profile(&profile_path)?;

        Ok(())
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<()> {
        let profile_path = Self::get_shell_profile_path()?;
        Self::add_path_entry(&profile_path, sdk_path)?;
        info!("success add `{}` to path!", sdk_path);

        // Source the profile to apply changes to current process
        Self::source_profile(&profile_path)?;

        Ok(())
    }

    fn get_path(&self) -> Result<String> {
        let path = env::var(ENV_PATH).unwrap_or_default();
        Ok(path)
    }
}
