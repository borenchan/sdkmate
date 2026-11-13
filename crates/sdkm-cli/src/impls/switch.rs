use crate::CommandHandler;
use crate::terminal::prompt_confirm;
use crate::terminal_sink::manager;
use anyhow::bail;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct SwitchHandler {
    /// SDK name. Built-in: java, node, python, go, maven, ... (see docs); custom SDKs from config.toml
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, go, maven, ... (see docs). Custom SDKs from config.toml also accepted"
    )]
    sdk: String,

    /// Target version (fuzzy match supported, e.g. '21' resolves to latest 21.x; must be installed locally)
    #[arg(
        value_name = "version",
        help = "Target version (fuzzy match supported, e.g. '21' → latest 21.x; must be installed locally)"
    )]
    sdk_version: String,
}

impl CommandHandler for SwitchHandler {
    fn run(&self) -> anyhow::Result<()> {
        let mut manager = manager()?;
        let sdk = manager.match_valid_sdk(&self.sdk)?;
        // 模糊命中时确认（文案逐字迁自原 core 内嵌 prompt）；TUI 路径选精确版本无确认
        let resolved = manager.resolve_installed_version(&sdk, &self.sdk_version)?;
        if resolved.fuzzy_matched {
            let confirmed = prompt_confirm(&format!(
                "Resolved '{}' → '{}'. Switch to this version?",
                self.sdk_version, resolved.version
            ))?;
            if !confirmed {
                bail!("Switch cancelled by user");
            }
        }
        manager.switch_sdk_to_version(&sdk, &resolved.version)?;
        Ok(())
    }
}
