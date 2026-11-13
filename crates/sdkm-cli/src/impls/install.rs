use crate::CommandHandler;
use crate::terminal::prompt_confirm;
use crate::terminal_sink::manager;
use anyhow::bail;
use clap::Parser;
use sdkm_core::manager::SdkManager;
use sdkm_util::sdk::Sdk;

#[derive(Debug, Parser)]
pub struct InstallHandler {
    /// SDK name. Built-in: java, node, python, go, maven, ... (see docs); custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, go, maven, ... (see docs). Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version (fuzzy match supported, e.g. '21' resolves to latest 21.x)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// Do not auto-switch to the installed version
    #[arg(long, help = "Do not auto-switch to the installed version")]
    no_switch: bool,
}

impl CommandHandler for InstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = manager()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        let auto_switch = !self.no_switch;
        install_with_confirm(&mut manager, &sdk, &self.sdk_version, auto_switch)
    }
}

/// 安装统一入口：解析 → 模糊命中时确认（文案逐字迁自原 core 内嵌 prompt）→ 执行安装。
/// CLI handler 与 TUI 动作共用（TUI 选的是精确版本，fuzzy=false 不会触发确认）
pub fn install_with_confirm(
    manager: &mut SdkManager,
    sdk: &Sdk,
    version_input: &str,
    auto_switch: bool,
) -> anyhow::Result<()> {
    let resolved = manager.resolve_install_version(sdk, version_input)?;
    if resolved.fuzzy_matched {
        let confirmed = prompt_confirm(&format!(
            "Resolved '{}' → '{}'. Install this version?",
            version_input, resolved.full_version
        ))?;
        if !confirmed {
            bail!("Installation cancelled by user");
        }
    }
    manager.install_sdk(sdk, &resolved, auto_switch)
}
