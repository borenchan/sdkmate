use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use crate::manager::SdkManager;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::rc::Rc;
use toml::to_string;
use util::config_helper::PLACEHOLDER_SDK_DIR;
use util::consts::SDKM_SYMLINK_DIR;
use util::sdk::Sdk;
use util::success;
use crate::link::symlink::create_symlink;
use crate::manager::config::SdkConfig;

impl SdkManager {
    pub fn switch_sdk_to_version(&mut self, sdk: &Sdk,sdk_version: &str) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let sdk_conf = self.config.find_sdk_ok(&sdk)?;
        let is_active = versions.iter().any(|v| v.is_active && v.sdk_version==sdk_version);
        if !is_active {
            let current_version_sdk = versions.into_iter().find(|v| v.sdk_version == sdk_version).context(format!("not found `{sdk}` version `{sdk_version}`, please check sdk's dir!"))?;
            let symlink_root_dir = self.config.symlink_dir.clone();
            let sdk_symlink_dir = PathBuf::from(symlink_root_dir).join(sdk.to_string());
            create_symlink(&current_version_sdk.sdk_dir,&sdk_symlink_dir)?;
            let sdk_symlink_bin_dir = sdk_symlink_dir.join(sdk_conf.bin_dir.as_str());
            // add sdk symlink link to current active version dir
            let sdk_symlink_bin_cow = sdk_symlink_bin_dir.to_string_lossy();
            let path = self.env_operation.get_path()?;
            // add sdk path only when does not exist in the os path
            if !path.contains(sdk_symlink_bin_cow.as_ref()) {
                self.env_operation.set_sdk_envs(&Self::get_sdk_extra_envs(sdk_conf, sdk_symlink_dir)?)?;
                self.env_operation.add_sdk_path(sdk_symlink_bin_cow.as_ref())?;
            }
            //todo error restore link and path
            //success switch version, need update config
            let sdk_conf = self.config.find_sdk_mut_ok(&sdk)?;
            sdk_conf.current_version = Some(sdk_version.to_string());
            self.config.write_to_disk()?;
        }
        success!("switch sdk `{}` to version `{}` success!", sdk, sdk_version);
        Ok(())
    }

    fn get_sdk_extra_envs(sdk_conf: &SdkConfig, sdk_symlink_dir: PathBuf) -> Result<HashMap<String, String>> {
        let mut env = HashMap::with_capacity(1);
        let sdk_dir = sdk_symlink_dir.to_string_lossy();
        env.insert(PLACEHOLDER_SDK_DIR, sdk_dir.as_ref());
        let actual_extra_vars = sdk_conf.get_actual_extra_vars(&env)?;
        Ok(actual_extra_vars)
    }
}
