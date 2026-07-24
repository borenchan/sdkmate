use crate::CommandHandler;
use crate::tui::{SelectorAction, run_local_selector, run_remote_selector};
use anyhow::{Result, bail};
use clap::Parser;
use sdkcore::list::RemoteVersionResult;
use sdkcore::manager::SdkManager;
use util::sdk::Sdk;

#[derive(Debug, Parser)]
pub struct ListHandler {
    /// SDK name. Built-in: java, node, python, maven, go; omit to list all installed SDKs
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven, go. Omit to list all installed SDKs"
    )]
    sdk: Option<String>,

    /// Fetch versions from remote (requires SDK name)
    #[arg(
        short = 'r',
        long = "remote",
        help = "Fetch versions from remote (requires SDK name)"
    )]
    remote: bool,

    /// Max remote versions to display (default: 20)
    #[arg(
        short,
        long,
        default_value_t = 20,
        help = "Max remote versions to display (default: 20)"
    )]
    limit: u32,
}

impl CommandHandler for ListHandler {
    fn run(&self) -> Result<()> {
        let manager = SdkManager::new()?;

        if self.limit == 0 {
            bail!("limit must be >= 1");
        }

        if self.remote && self.sdk.is_none() {
            bail!("Please specify an SDK name, e.g.: sdkm list <sdk> -r");
        }

        match (&self.sdk, self.remote) {
            // 无 SDK → 打印所有已安装概览
            (None, false) => manager.show_local_sdk_list(),
            // 本地 SDK → 交互式版本选择器
            (Some(sdk_name), false) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                let versions = manager.list_local_sdk_versions(&sdk)?;
                let action = run_local_selector(&sdk.to_string(), &versions)?;
                self.execute_action(action, &sdk)
            }
            // 远程 SDK → spinner + 交互式版本选择器
            (Some(sdk_name), true) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                let RemoteVersionResult { items, total_count } = manager.fetch_remote_version_list(&sdk, self.limit)?;
                let action = run_remote_selector(&sdk.to_string(), &items, self.limit, total_count)?;
                self.execute_action(action, &sdk)
            }
            (None, true) => unreachable!(),
        }
    }
}

impl ListHandler {
    fn execute_action(&self, action: SelectorAction, sdk: &Sdk) -> Result<()> {
        match action {
            SelectorAction::Quit => Ok(()),
            SelectorAction::Install { version } => {
                let mut manager = SdkManager::new()?;
                manager.install_sdk(sdk, &version, true)
            }
            SelectorAction::Switch { version } => {
                let mut manager = SdkManager::new()?;
                manager.switch_sdk_to_version(sdk, &version)
            }
            SelectorAction::Uninstall { version } => {
                let mut manager = SdkManager::new()?;
                // yes=false：TUI 退出后在正常终端走 uninstall 的二次确认（破坏性操作）
                manager.uninstall_sdk(sdk, &version, false)
            }
        }
    }
}
