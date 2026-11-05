use crate::CommandHandler;
use crate::terminal_sink::manager;
use anyhow::bail;
use clap::Parser;
use sdkcore::manager::SdkManager;
use sdkcore::uninstall::ResolvedUninstall;
use util::sdk::Sdk;
use util::terminal::prompt_confirm;

#[derive(Debug, Parser)]
pub struct UninstallHandler {
    /// SDK name. Built-in: java, node, python, go, maven, ... (see docs); custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, go, maven, ... (see docs). Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version to uninstall (fuzzy match supported, e.g. '21' resolves to latest 21.x)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x)"
    )]
    sdk_version: String,

    /// Skip all interactive confirmations
    #[arg(short = 'y', long, help = "Skip all interactive confirmations")]
    yes: bool,
}

impl CommandHandler for UninstallHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = manager()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        uninstall_with_confirm(&mut manager, &sdk, &self.sdk_version, self.yes)
    }
}

/// 卸载统一入口：目标预检 → 破坏性确认（-y 跳过；文案逐字迁自原 core 内嵌 prompt）→ 执行卸载。
/// CLI handler 与 TUI 动作共用（TUI 恒走确认——破坏性操作二次确认语义与原 core yes=false 一致）
pub fn uninstall_with_confirm(
    manager: &mut SdkManager,
    sdk: &Sdk,
    version_input: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let target = manager.resolve_uninstall_target(sdk, version_input)?;
    if !yes {
        let confirmed = prompt_confirm(&uninstall_confirm_prompt(sdk, version_input, &target))?;
        if !confirmed {
            bail!("Uninstall cancelled by user");
        }
    }
    manager.uninstall_sdk(sdk, &target.version)
}

/// 卸载确认文案（逐字迁自原 core：解析前缀 + 三分支动作说明 + Continue?）
fn uninstall_confirm_prompt(sdk: &Sdk, version_input: &str, t: &ResolvedUninstall) -> String {
    let resolved = if t.fuzzy_matched {
        format!("Resolved '{}' → '{}'. ", version_input, t.version)
    } else {
        String::new()
    };
    let action = match (&t.other_version, t.is_active) {
        (Some(other), true) => format!(
            "`{}` `{}` is the active version. sdkm will switch to `{}` first, then uninstall.",
            sdk, t.version, other
        ),
        (None, true) => format!(
            "`{}` `{}` is the only installed version. sdkm will clean up symlink, PATH, env vars and current_version, then remove it.",
            sdk, t.version
        ),
        _ => format!(
            "sdkm will uninstall `{}` `{}` (remove the version directory only).",
            sdk, t.version
        ),
    };
    format!("{resolved}{action} Continue?")
}
