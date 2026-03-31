use anyhow::Result;
use crossterm::style::Stylize;
use std::{env, fs};
use std::path::{Path, PathBuf};
use util::consts::{SDKM_SYMLINK_DIR, SDKM_STORE_DIR};
use util::{info, success};
use crate::env::{EnvOperation, OsEnvOperation};
use crate::link::symlink::create_symlink;
use crate::manager::config::{SdkmConfig, CONFIG_FILE_NAME};
use crate::manager::SdkManager;

impl SdkManager {
    /// init sdkm
    pub fn init_sdkm(force:  bool) -> Result<()> {
        info!("initializing sdkm...");
        let root_dir = env::current_dir()?;
        //0. add sdkm cli to path
        let os = OsEnvOperation{};
        os.add_sdk_path(root_dir.display().to_string().as_str())?;
        //1. create sdk store root dir
        let sdks_dir = root_dir.join(SDKM_STORE_DIR);
        if !sdks_dir.exists() {
            fs::create_dir(&sdks_dir)?
        }
        //2. create sdkm config file
        Self::init_sdkm_config_file(&root_dir, force)?;
        info!("sdkm root dir: {}  , sdks store dir:{}", root_dir.display(),sdks_dir.display());
        let config = SdkmConfig::read_from_disk()?;
        let sdkm_symlink_dir = config.symlink_dir.unwrap_or(SDKM_SYMLINK_DIR.to_string());
        //3. create sdkm symlink root dir
        fs::create_dir_all(sdkm_symlink_dir)?;
        success!("sdkm initialization is successful");
        Ok(())
    }
    ///
    /// `force`: if true, will overwrite the config file
    ///
    fn init_sdkm_config_file(root_dir: &PathBuf,force:  bool)->Result<()> {
        let config_file = root_dir.join(CONFIG_FILE_NAME);
        if !config_file.exists() || force {
            let mut config = SdkmConfig::default();
            config.install_dir = Some(env::current_dir()?.to_string_lossy().to_string());
            config.write_to_disk()?;
        }
        Ok(())
    }
}



