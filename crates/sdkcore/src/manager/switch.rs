use crate::manager::SdkManager;
use anyhow::{Context, Result};
use std::path::PathBuf;
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
            let symlink_root_dir = self.config.sdkm_symlink_dir.clone().unwrap_or(SDKM_SYMLINK_DIR.to_string());
            let sdk_symlink_dir = sdk.get_sdk_bin_dir(PathBuf::from(symlink_root_dir).join(sdk.to_string()));
            // add sdk symlink link to current active version dir
            let sdk_symlink_cow = sdk_symlink_dir.to_string_lossy();
            create_symlink(&current_version_sdk.sdk_dir,&sdk_symlink_dir)?;
            let path = self.env_operation.get_path()?;
            // add sdk path only when does not exist in the os path
            if !path.contains(sdk_symlink_cow.as_ref()) {
                self.env_operation.add_sdk_path(sdk_symlink_cow.as_ref())?;
            }
            //todo error restore link and path
        }
        success!("switch sdk `{}` to version `{}` success!", sdk, sdk_version);
        Ok(())
    }
}
