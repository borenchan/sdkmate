use std::ops::{Deref, DerefMut};
use crate::manager::SdkManager;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::rc::Rc;
use toml::to_string;
use util::consts::SDKM_SYMLINK_DIR;
use util::sdk::Sdk;
use util::success;
use crate::link::symlink::create_symlink;

impl SdkManager {
    pub fn switch_sdk_to_version(&mut self, sdk: Sdk,sdk_version: &str) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let is_active = versions.iter().any(|v| v.is_active && v.sdk_version==sdk_version);
        if !is_active {
            let current_version_sdk = versions.into_iter().find(|v| v.sdk_version == sdk_version).context(format!("not found `{sdk}` version `{sdk_version}`, please check sdk's dir!"))?;
            let symlink_root_dir = self.config.symlink_dir.clone();
            let symlink_sdk_dir = PathBuf::from(symlink_root_dir).join(sdk.clone().to_string());
            create_symlink(&current_version_sdk.sdk_dir,&symlink_sdk_dir)?;
            let sdk_bin_symlink_dir = sdk.get_sdk_bin_dir(&symlink_sdk_dir);
            // add sdk symlink link to current active version dir
            let sdk_bin_symlink_cow = sdk_bin_symlink_dir.to_string_lossy();
            let path = self.env_operation.get_path()?;
            // add sdk path only when does not exist in the os path
            if !path.contains(sdk_bin_symlink_cow.as_ref()) {
                self.env_operation.set_sdk_env(sdk,symlink_sdk_dir.to_string_lossy().as_ref())?;
                self.env_operation.add_sdk_path(sdk_bin_symlink_cow.as_ref())?;
            }
            //todo error restore link and path
        }
        success!("switch sdk `{}` to version `{}` success!", sdk, sdk_version);
        Ok(())
    }
}
