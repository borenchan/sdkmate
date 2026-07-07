use crate::config::SdkmConfig;
use crate::env::{EnvOperation, OsEnvOperation};
use crate::manager::SdkManager;
use anyhow::Result;
use std::env;
use std::fs;
use util::consts::{
    BANNER, CONFIG_FILE_NAME, DIR_DESC_CACHE, DIR_DESC_CONFIG, DIR_DESC_LINKS, DIR_DESC_STORE,
    DIR_DESC_TMP, SDKM_CACHE_DIR, SDKM_STORE_DIR, SDKM_TMP_DIR,
};
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home, is_sdkm_dedicated_dir};
use util::terminal::{prompt_confirm, suggest_sdkm_path};
use util::{banner, detail, divider, info, step, success, tree, try_bug, warning};

impl SdkManager {
    /// 初始化 sdkm
    ///
    /// - **非 force 模式不覆盖现有 config**：config.toml 已存在时只补建缺失目录，不重置
    ///   用户配置。用户改了 symlink_dir 后重跑 init 能补建 symlink 目录，不死循环。
    /// - **force 模式才写默认 config**：用户明确要求重置时覆盖。
    /// - **symlink_dir 跟随 home**：config 里留空 → 运行时 resolve 成 `<home>/links`；
    ///   不可写时 `bail!` 提示 `sdkm config set symlink_dir <可写目录>`。
    pub fn init_sdkm(force: bool) -> Result<()> {
        let root_dir = get_sdkm_home()?;
        let config_file = get_sdkm_config_path()?;
        let config_existed = config_file.exists();

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

        // ── force 模式确认覆盖现有 config ──
        if force && config_existed {
            warning!("Forced reinitialization — config will be overwritten.");
            detail!("Config file: {}", config_file.display());
            if !prompt_confirm("Continue?")? {
                info!("Operation aborted.");
                return Ok(());
            }
        }

        // ── 非首次 init 提示（不 return，继续补建缺失目录）──
        if config_existed && !force {
            warning!("sdkm is already initialized; ensuring directories exist.");
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

        step!("3/4", "Preparing config file");
        // 首次 init 或 force 重置：写默认 config；非 force 且已存在：保留用户配置不覆盖
        if !config_existed || force {
            try_bug!(Self::init_sdkm_config());
        }
        detail!("{} — {}", config_file.display(), DIR_DESC_CONFIG);

        step!("4/4", "Creating symlink directory");
        let config = try_bug!(SdkmConfig::read_from_disk());
        let symlink_dir = try_bug!(config.resolved_symlink_dir());
        if let Err(e) = fs::create_dir_all(&symlink_dir) {
            // 权限/路径不可写时明确引导，不报裸 Permission denied；属用户环境问题，非 bug
            anyhow::bail!(
                "Failed to create symlink directory '{}': {}\n  Tip: run `sdkm config set symlink_dir <writable_dir>` to use a custom location, then `sdkm init` again.",
                symlink_dir, e
            );
        }
        detail!("{} — active SDK bin links for PATH resolution", symlink_dir);

        // ── 目录树：透明展示 sdkm home 结构 ──
        let exe_name = env::current_exe()
            .ok()
            .and_then(|p| p.file_name().and_then(|f| f.to_str().map(String::from)))
            .unwrap_or_else(|| "sdkm".to_string());

        // symlink_dir 显示：在 home 下用相对名，自定义到 home 外用绝对路径标注
        let symlink_path = std::path::PathBuf::from(&symlink_dir);
        let symlink_display = if symlink_path.starts_with(&root_dir) {
            format!("{}/", symlink_path.strip_prefix(&root_dir).unwrap().display())
        } else {
            format!("{} (custom)", symlink_dir)
        };

        divider!();
        banner!("{}", BANNER.trim_start_matches('\n').trim_end_matches('\n'));
        tree!("{}  ← sdkm home", root_dir.display());
        tree!("├── {}  ← main program", exe_name);
        tree!("├── {}  ← {}", CONFIG_FILE_NAME, DIR_DESC_CONFIG);
        tree!("├── {}/  ← {}", SDKM_STORE_DIR, DIR_DESC_STORE);
        tree!("├── {}  ← {}", symlink_display, DIR_DESC_LINKS);
        tree!("├── {}/  ← {}", SDKM_CACHE_DIR, DIR_DESC_CACHE);
        tree!("└── {}/  ← {}", SDKM_TMP_DIR, DIR_DESC_TMP);

        divider!();
        success!("Congratulations! sdkm initialized successfully!");
        success!("Run `sdkm install java 21` to start using it. Restart your terminal for `sdkm` to take effect in PATH.");
        Ok(())
    }

    /// 创建默认配置文件
    fn init_sdkm_config() -> Result<()> {
        let config = SdkmConfig::default();
        config.write_to_disk()?;
        Ok(())
    }
}
