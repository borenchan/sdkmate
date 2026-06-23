use crate::env::{EnvOperation, OsEnvOperation};
use crate::manager::SdkManager;
use crate::config::SdkmConfig;
use anyhow::Result;
use std::env;
use std::fs;
use util::consts::{
    BANNER, CONFIG_FILE_NAME, DIR_DESC_CACHE, DIR_DESC_CONFIG, DIR_DESC_STORE, DIR_DESC_TMP, SDKM_CACHE_DIR,
    SDKM_STORE_DIR, SDKM_TMP_DIR,
};
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home, is_sdkm_dedicated_dir};
use util::terminal::{prompt_confirm, suggest_sdkm_path};
use util::{banner, detail, divider, info, step, success, tree, try_bug, warning};

impl SdkManager {
    /// 初始化 sdkm
    pub fn init_sdkm(force: bool) -> Result<()> {
        let root_dir = get_sdkm_home()?;

        // ── 目录检测：仅在非 force 模式下检查 ──
        if !force && !is_sdkm_dedicated_dir(&root_dir) {
            warning!("Current directory may not be a dedicated sdkm folder.");
            detail!("Current: {}", root_dir.display());
            detail!("Suggested: {}", suggest_sdkm_path());
            detail!("Move sdkm.exe there and run `sdkm init` again.");
            if !prompt_confirm("Continue initializing here anyway?")? {
                info!("Operation aborted.");
                return Ok(());
            }
        }

        // ── 配置文件存在检查 ──
        let config_file = get_sdkm_config_path()?;
        if config_file.exists() {
            if force {
                warning!("Forced reinitialization — config will be overwritten.");
                detail!("Config file: {}", config_file.display());
                if !prompt_confirm("Continue?")? {
                    info!("Operation aborted.");
                    return Ok(());
                }
            } else {
                warning!("sdkm is already initialized.");
                info!("To reinitialize, run: `sdkm init --force`");
                return Ok(());
            }
        }

        // ── 开始初始化 ──
        divider!();

        step!("1/4", "Creating store directory");
        let sdks_dir = get_installed_sdks_dir()?;
        if !sdks_dir.exists() {
            try_bug!(fs::create_dir(&sdks_dir));
        }
        detail!("{} — {}", sdks_dir.display(), DIR_DESC_STORE);

        step!("2/4", "Adding sdkm to system PATH");
        detail!("{} — sdkm CLI accessible from any terminal", root_dir.display());
        let os = OsEnvOperation {};
        try_bug!(os.add_sdk_path(root_dir.to_string_lossy().as_ref()));

        step!("3/4", "Creating config file");
        try_bug!(Self::init_sdkm_config());
        detail!("{} — {}", config_file.display(), DIR_DESC_CONFIG);

        step!("4/4", "Creating symlink directory");
        let config = try_bug!(SdkmConfig::read_from_disk());
        let symlink_dir = config.symlink_dir;
        try_bug!(fs::create_dir_all(&symlink_dir));
        detail!("{} — active SDK bin links for PATH resolution", symlink_dir);

        // ── 目录树：透明展示 sdkm home 结构 ──
        let exe_name = env::current_exe()
            .ok()
            .and_then(|p| p.file_name().and_then(|f| f.to_str().map(String::from)))
            .unwrap_or_else(|| "sdkm".to_string());

        divider!();
        banner!("{}", BANNER.trim_start_matches('\n').trim_end_matches('\n'));
        tree!("{}  ← sdkm home", root_dir.display());
        tree!("├── {}  ← main program", exe_name);
        tree!("├── {}  ← {}", CONFIG_FILE_NAME, DIR_DESC_CONFIG);
        tree!("├── {}/  ← {}", SDKM_STORE_DIR, DIR_DESC_STORE);
        tree!("├── {}/  ← {}", SDKM_CACHE_DIR, DIR_DESC_CACHE);
        tree!("└── {}/  ← {}", SDKM_TMP_DIR, DIR_DESC_TMP);

        divider!();
        success!("Congratulations! sdkm initialized successfully!");
        success!("Run `sdkm install java 21` to start using it");
        Ok(())
    }

    /// 创建默认配置文件
    fn init_sdkm_config() -> Result<()> {
        let config = SdkmConfig::default();
        config.write_to_disk()?;
        Ok(())
    }
}
