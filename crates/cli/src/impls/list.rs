use crate::CommandHandler;
use crate::tui::{SelectorAction, run_local_selector, run_remote_selector};
use anyhow::{bail, Result};
use clap::Parser;
use sdkcore::manager::SdkManager;
use sdkcore::manager::list::RemoteVersionResult;
use util::sdk::Sdk;

#[derive(Debug, Parser)]
pub struct ListHandler {
    /// 要查询的 SDK 名称。内置: java, node, python, maven; 留空则显示全部已安装 SDK
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven. Omit to list all installed SDKs"
    )]
    sdk: Option<String>,

    /// 从远程服务器获取版本列表，需要指定 SDK 名称
    #[arg(short = 'r', long = "remote", help = "Fetch versions from remote (requires SDK name)")]
    remote: bool,

    /// 远程版本显示上限，默认 20
    #[arg(short, long, default_value_t = 20, help = "Max remote versions to display (default: 20)")]
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
        }
    }
}
