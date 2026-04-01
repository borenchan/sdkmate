use crate::env::{EnvOperation, OsEnvOperation};
use crate::manager::config::SdkmConfig;
use crate::manager::SdkManager;
use anyhow::Result;
use std::fs;
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};
use util::{info, success, warning};
use util::terminal::{info, prompt_confirm};

impl SdkManager {
    /// init sdkm
    pub fn init_sdkm(force:  bool) -> Result<()> {
        let config_file = get_sdkm_config_path()?;
        if config_file.exists() {
            if force {
                warning!("You are performing sdkm forced reinitialization operation!");
                warning!("sdkm config initialized:`{}`, will overwrite", config_file.display());
                if !prompt_confirm("Continue?")? {
                    info("Operation Aborted.");
                    return Ok(())
                }
            }else {
                warning!("sdkm is already initialized.");
                info!("if you want to reinitialize, run: `sdkm init --force`");
                return Ok(())
            }
        }
        info!("initializing sdkm...");
        let root_dir = get_sdkm_home()?;
        let sdks_dir = get_installed_sdks_dir()?;
        info!("sdkm home dir: `{}` sdks store dir: `{}`", root_dir.display(),sdks_dir.display());
        // create sdk store root dir
        if !sdks_dir.exists() {
            fs::create_dir(&sdks_dir)?
        }
        //add sdkm cli to path
        let os = OsEnvOperation{};
        os.add_sdk_path(root_dir.to_string_lossy().as_ref())?;
        //create sdkm config file
        Self::init_sdkm_config()?;
        let config = SdkmConfig::read_from_disk()?;
        let sdkm_symlink_dir = config.symlink_dir;
        //create sdkm symlink root dir
        fs::create_dir_all(sdkm_symlink_dir)?;
        success!("sdkm initialization is successful");
        Ok(())
    }


    ///
    /// init sdkm config file, use default config value
    ///
    fn init_sdkm_config() ->Result<()> {
        let mut config = SdkmConfig::default();
        // config.home_dir = get_sdkm_home().map_or(|p| Some(p.to_string_lossy().to_string()));
        config.write_to_disk()?;
        Ok(())
    }

}



