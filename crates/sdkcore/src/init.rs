//! `sdkm init` 业务：前置检查（InitCheck，确认交互归调用端）+ 执行段（过程消息经 sink
//! 发事件）+ 结果数据（InitSummary，目录树等装饰渲染归前端）。
//!
//! 保持关联函数形态（不经 SdkManager::new）：init 是首次创建 config 的命令，
//! config 未建时 read_from_disk 会失败，manager() 装配路径不可用。

use crate::config::SdkmConfig;
use crate::env::{EnvOperation, OsEnvOperation, report_env_actions};
use crate::manager::SdkManager;
use crate::report::EventSink;
use crate::shell::inject;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use util::consts::{DIR_DESC_CONFIG, DIR_DESC_STORE};
use util::path::{
    get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home, is_sdkm_dedicated_dir, suggest_sdkm_path,
};
use util::region::is_china_mainland_timezone;
use util::try_bug;

/// init 前置检查结果（只读零副作用）：调用端据此组装确认交互文案，确认后执行 init_sdkm
pub struct InitCheck {
    /// sdkm home 目录
    pub root_dir: PathBuf,
    /// config 文件路径
    pub config_file: PathBuf,
    /// config.toml 是否已存在
    pub config_existed: bool,
    /// 非 force 且当前目录不是专用 sdkm 目录 → 需"就地初始化"确认（携带建议路径）
    pub dir_confirm: Option<String>,
    /// force 且 config 已存在 → 需"覆盖 config"确认
    pub overwrite_confirm: bool,
}

/// init 结果数据（目录树展示所需），装饰渲染（banner/tree/divider/success）归前端
pub struct InitSummary {
    /// sdkm home 目录
    pub home: PathBuf,
    /// 运行中 exe 文件名（main program 行）
    pub exe_name: String,
    /// symlink_dir 展示名：home 内相对名（`links/`）或绝对路径 + ` (custom)` 标注
    pub symlink_display: String,
}

/// 前置检查（只读）：目录检测 + config 存在性，产出确认决策数据（core 零 stdin）
pub fn check_init_env(force: bool) -> Result<InitCheck> {
    let root_dir = get_sdkm_home()?;
    let config_file = get_sdkm_config_path()?;
    let config_existed = config_file.exists();

    // 非 force 且非专用 sdkm 目录 → 需确认（建议路径数据随返回值）
    let dir_confirm = if !force && !is_sdkm_dedicated_dir(&root_dir) {
        Some(suggest_sdkm_path())
    } else {
        None
    };

    Ok(InitCheck {
        root_dir,
        config_file,
        config_existed,
        dir_confirm,
        overwrite_confirm: force && config_existed,
    })
}

impl SdkManager {
    /// 初始化 sdkm（执行段，确认交互已由调用端完成）
    ///
    /// - **非 force 模式不覆盖现有 config**：config.toml 已存在时只补建缺失目录，不重置
    ///   用户配置。用户改了 symlink_dir 后重跑 init 能补建 symlink 目录，不死循环。
    /// - **force 模式才写默认 config**：用户明确要求重置时覆盖。
    /// - **symlink_dir 跟随 home**：config 里留空 → 运行时 resolve 成 `<home>/links`；
    ///   不可写时 `bail!` 提示 `sdkm config set symlink_dir <可写目录>`。
    ///
    /// 过程消息经 sink 发事件；目录树等结果数据随 InitSummary 返回（装饰归前端）。
    pub fn init_sdkm(sink: Arc<dyn EventSink>, force: bool) -> Result<InitSummary> {
        let check = check_init_env(force)?;
        let root_dir = check.root_dir;
        let config_existed = check.config_existed;

        // ── 非首次 init 提示（不 return，继续补建缺失目录）──
        if config_existed && !force {
            sink.warning("sdkm is already initialized; ensuring directories exist.".to_string());
        }

        // ── 开始初始化 ──
        sink.step("1/5".to_string(), "Creating store directory".to_string());
        let sdks_dir = get_installed_sdks_dir()?;
        if !sdks_dir.exists() {
            try_bug!(fs::create_dir(&sdks_dir));
        }
        sink.detail(format!("{} — {}", sdks_dir.display(), DIR_DESC_STORE));

        sink.step("2/5".to_string(), "Adding sdkm to system PATH".to_string());
        sink.detail(format!("{} — sdkm CLI accessible from any terminal", root_dir.display()));
        let os = OsEnvOperation {};
        let actions = os
            .add_sdk_path(root_dir.to_string_lossy().as_ref())
            // 写系统 PATH 需管理员权限（HKLM），属用户环境问题，非 bug
            .map_err(|e| anyhow::anyhow!("Failed to add sdkm to system PATH: {}", e))?;
        report_env_actions(sink.as_ref(), &actions);

        sink.step("3/5".to_string(), "Preparing config file".to_string());
        // 首次 init 或 force 重置：写默认 config；非 force 且已存在：保留用户配置不覆盖
        if !config_existed || force {
            try_bug!(Self::init_sdkm_config(sink.as_ref()));
        }
        sink.detail(format!("{} — {}", check.config_file.display(), DIR_DESC_CONFIG));

        sink.step("4/5".to_string(), "Creating symlink directory".to_string());
        let config = try_bug!(SdkmConfig::read_from_disk());
        let symlink_dir = try_bug!(config.resolved_symlink_dir());
        if let Err(e) = fs::create_dir_all(&symlink_dir) {
            // 权限/路径不可写时明确引导，不报裸 Permission denied；属用户环境问题，非 bug
            anyhow::bail!(
                "Failed to create symlink directory '{}': {}\n  Tip: run `sdkm config set symlink_dir <writable_dir>` to use a custom location, then `sdkm init` again.",
                symlink_dir,
                e
            );
        }
        sink.detail(format!("{} — active SDK bin links for PATH resolution", symlink_dir));

        sink.step("5/5".to_string(), "Injecting shell hook".to_string());
        // shell 注入细节在 shell/inject.rs（检测/定位/去重/升级），init 只编排
        inject::inject_shell_hook(sink.as_ref())?;

        // ── 目录树展示数据（渲染归前端）──
        let exe_name = env::current_exe()
            .ok()
            .and_then(|p| p.file_name().and_then(|f| f.to_str().map(String::from)))
            .unwrap_or_else(|| "sdkm".to_string());

        // symlink_dir 显示：在 home 下用相对名，自定义到 home 外用绝对路径标注
        let symlink_path = PathBuf::from(&symlink_dir);
        let symlink_display = if symlink_path.starts_with(&root_dir) {
            format!("{}/", symlink_path.strip_prefix(&root_dir).unwrap().display())
        } else {
            format!("{} (custom)", symlink_dir)
        };

        Ok(InitSummary {
            home: root_dir,
            exe_name,
            symlink_display,
        })
    }

    /// 创建默认配置文件
    fn init_sdkm_config(sink: &dyn EventSink) -> Result<()> {
        let mut config = SdkmConfig::default();
        // 中国大陆时区 → 默认启用 GitHub 下载镜像（GH 资产国内直连基本不可用，
        // 镜像可随时 config set 清空关闭）；检测失败返回 false 不影响主流程
        if is_china_mainland_timezone() {
            config.network.github_download_mirror = Some("https://gh-proxy.com/".to_string());
            sink.success(
                "China mainland timezone detected — GitHub download mirror enabled by default (gh-proxy.com)"
                    .to_string(),
            );
            sink.detail("To disable: sdkm config set network.github_download_mirror \"\"".to_string());
        }
        config.write_to_disk()?;
        Ok(())
    }
}
