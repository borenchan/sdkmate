use crate::manager::SdkManager;
use anyhow::Result;
use util::info;
use util::path::get_installed_sdks_dir;
use util::sdk::Sdk;

impl SdkManager {
    pub fn query_local_sdk_list(&self) -> Result<()> {
        let sdk_dir = get_installed_sdks_dir()?;
        let mut i = 1;
        for entry in sdk_dir.read_dir()? {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    info!("{i}. {}", entry.file_name().display());
                    i += 1;
                }
            }
        }
        Ok(())
    }
    pub fn query_local_sdk_version_list(&self, sdk: Sdk) -> Result<()> {
        let sdks_dir = get_installed_sdks_dir()?;
        let mut i = 1;
        let sdk_dir = sdks_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy() == sdk.to_string());
        if let Some(sdk_dir) = sdk_dir {
            sdk_dir.path().read_dir()?
                .filter_map(|entry| entry.ok())
                .for_each(|sdk_version| {
                    info!("{i}. {}", sdk_version.file_name().display());
                    i += 1;
            });
        } else {
            return Err(anyhow::anyhow!(
                "sdk:`{}` not found in sdkm's dir `{}`",
                sdk,
                sdks_dir.display()
            ));
        }

        Ok(())
    }
}
