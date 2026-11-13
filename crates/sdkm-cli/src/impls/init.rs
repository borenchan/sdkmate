use crate::CommandHandler;
use crate::terminal::{divider, prompt_confirm};
// 确认段/装饰渲染走终端宏（crate root 导入），divider/prompt_confirm 用函数版
use crate::{banner, detail, info, success, tree, warning};
use anyhow::Result;
use clap::ArgAction::SetTrue;
use clap::Parser;
use sdkm_core::init::{InitSummary, check_init_env};
use sdkm_core::manager::SdkManager;
use sdkm_util::consts::{
    BANNER, CONFIG_FILE_NAME, DIR_DESC_CACHE, DIR_DESC_CONFIG, DIR_DESC_LINKS, DIR_DESC_STORE, DIR_DESC_TMP,
    SDKM_CACHE_DIR, SDKM_STORE_DIR, SDKM_TMP_DIR,
};
use std::sync::Arc;

#[derive(Debug, Parser)]
pub struct InitHandler {
    /// Force reinitialize sdkm (overwrites config, skips dir check)
    #[arg(
        long,
        short,
        default_value_t = false,
        action = SetTrue,
        help = "Force reinitialize sdkm (overwrites config, skips dir check)"
    )]
    force: bool,
}

impl CommandHandler for InitHandler {
    fn run(&self) -> Result<()> {
        let sink = Arc::new(crate::terminal_sink::TerminalSink::new());

        // ── 前置检查 + 确认交互（文案与原 core 版逐字一致）──
        let check = check_init_env(self.force)?;

        // 非 force 且非专用 sdkm 目录 → 确认就地初始化
        if let Some(suggested) = &check.dir_confirm {
            warning!("Current directory may not be a dedicated sdkm folder.");
            detail!("Current: {}", check.root_dir.display());
            detail!("Suggested: {}", suggested);
            detail!("Move sdkm.exe there and run `sdkm init` again.");
            if !prompt_confirm("Continue initializing here anyway?")? {
                info!("Operation aborted.");
                return Ok(());
            }
        }

        // force 且 config 已存在 → 确认覆盖
        if check.overwrite_confirm {
            warning!("Forced reinitialization — config will be overwritten.");
            detail!("Config file: {}", check.config_file.display());
            if !prompt_confirm("Continue?")? {
                info!("Operation aborted.");
                return Ok(());
            }
        }

        // ── 执行段（过程消息经 sink 发事件）──
        divider();
        let summary = SdkManager::init_sdkm(sink, self.force)?;

        // ── 结果装饰渲染（banner/目录树/success 归前端）──
        render_summary(&summary);
        Ok(())
    }
}

/// 目录树 + 成功文案渲染（数据来自 InitSummary，样式与原 core 版逐字节一致）
fn render_summary(summary: &InitSummary) {
    let InitSummary {
        home: root_dir,
        exe_name,
        symlink_display,
    } = summary;

    divider();
    banner!("{}", BANNER.trim_start_matches('\n').trim_end_matches('\n'));
    tree!("{}  ← sdkm home", root_dir.display());
    tree!("├── {}  ← main program", exe_name);
    tree!("├── {}  ← {}", CONFIG_FILE_NAME, DIR_DESC_CONFIG);
    tree!("├── {}/  ← {}", SDKM_STORE_DIR, DIR_DESC_STORE);
    tree!("├── {}  ← {}", symlink_display, DIR_DESC_LINKS);
    tree!("├── {}/  ← {}", SDKM_CACHE_DIR, DIR_DESC_CACHE);
    tree!("└── {}/  ← {}", SDKM_TMP_DIR, DIR_DESC_TMP);

    divider();
    success!("Congratulations! sdkm initialized successfully!");
    success!("Run `sdkm install java 21` to start using it. Restart your terminal for `sdkm` to take effect in PATH.");
}
