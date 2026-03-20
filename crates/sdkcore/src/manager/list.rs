use std::ffi::OsStr;
use std::path::PathBuf;
use crate::manager::SdkManager;
use anyhow::Result;
use util::info;
use util::path::get_installed_sdks_dir;
use util::sdk::Sdk;


#[derive(Debug)]
pub struct SdkVersionItem{
    pub sdk: Sdk,
    pub sdk_version: String,
    pub sdk_dir: PathBuf,
    pub is_active: bool,
}
impl SdkVersionItem {
    pub fn new(sdk: Sdk, sdk_dir: PathBuf, is_active: bool) -> Self {
        let cow = sdk_dir.file_name().unwrap_or(OsStr::new("(empty dir)")).to_string_lossy();
        Self {
            sdk,
            sdk_version: cow.to_string(),
            sdk_dir,
            is_active,
        }
    }

}
impl SdkManager {
    pub fn show_local_sdk_list(&self) -> Result<()> {
        let sdk_dir = get_installed_sdks_dir()?;
        let mut i = 1;
        for entry in sdk_dir.read_dir()? {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    info!("{i:>2}. {}", entry.file_name().display());
                    i += 1;
                }
            }
        }
        Ok(())
    }
    /// list specified sdk versions from local sdkm root dir
    pub fn list_local_sdk_versions(&self, sdk: Sdk) -> Result<Vec<SdkVersionItem>> {
        let sdks_root_dir = get_installed_sdks_dir()?;
        let sdk_dir = sdks_root_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy() == sdk.to_string());
        if let Some(sdk_dir) = sdk_dir {
            let result = sdk_dir.path().read_dir()?
                .filter_map(|entry| entry.ok())
                .map(|sdk_version| {
                    let sdk_version_dir = sdk_version.path();
                    // &self.config.java.unwrap().current_active_version
                    SdkVersionItem::new(sdk, sdk_version_dir, false)
                })
                .collect();
            Ok(result)
        } else {
            Err(anyhow::anyhow!(
                "sdk:`{}` not found in sdkm's dir `{}`",
                sdk,
                sdks_root_dir.display()
            ))
        }
    }
    pub fn show_local_sdk_version_list(&self, sdk: Sdk) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let mut i = 1;
        versions.iter().for_each(|sdk_version| {
            info!("{} {i:>2}. {} ", if sdk_version.is_active {"✅"} else {""}, sdk_version.sdk_version);
            i += 1;
        });
        Ok(())
    }
}
