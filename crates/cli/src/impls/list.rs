use crate::CommandHandler;
use crate::tui::{SdkSelectorAction, SelectorAction, run_local_selector, run_remote_selector, run_sdk_selector};
use anyhow::{Result, bail};
use clap::Parser;
use sdkcore::list::{InstallStatus, RemoteVersionItem, RemoteVersionResult};
use sdkcore::manager::SdkManager;
use std::io::IsTerminal;
use util::sdk::Sdk;

#[derive(Debug, Parser)]
pub struct ListHandler {
    /// SDK name. Built-in: java, node, python, maven, go; omit to browse all registered SDKs
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, maven, go. Omit to browse all registered SDKs (TUI)"
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
            // 无 SDK → TTY 走两层 SDK/版本选择 TUI；非 TTY（管道/agent）打印已安装概览
            (None, false) => {
                if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                    self.run_sdk_browse_tui()
                } else {
                    manager.show_local_sdk_list()
                }
            }
            // 本地 SDK → TTY 交互式版本选择器；非 TTY 文本列表
            (Some(sdk_name), false) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                let versions = manager.list_local_sdk_versions(&sdk)?;
                if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                    let action = run_local_selector(&sdk.to_string(), &versions)?;
                    self.execute_action(action, &sdk)
                } else {
                    manager.show_local_sdk_version_list(&sdk)
                }
            }
            // 远程 SDK → TTY spinner + 交互式版本选择器；非 TTY 文本列表
            (Some(sdk_name), true) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                let RemoteVersionResult { items, total_count } = manager.fetch_remote_version_list(&sdk, self.limit)?;
                if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                    let action = run_remote_selector(&sdk.to_string(), &items, self.limit, total_count)?;
                    self.execute_action(action, &sdk)
                } else {
                    Self::print_remote_text(&items)
                }
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

    /// 两层浏览 TUI：第一层 SDK 选择 → 第二层本地/远程版本选择 → q 回第一层循环
    ///
    /// 每轮回第一层重建 manager 重读 config + 重收 SDK 列表，装完/切完状态即时刷新；
    /// 拉取失败等反馈经 msg 传回第一层 TUI 的持久消息区显示。
    fn run_sdk_browse_tui(&self) -> Result<()> {
        let mut msg = String::new();
        loop {
            // 每轮重建：execute_action 里的安装/切换写盘后，旧 manager 的 config 内存快照已过期
            let manager = SdkManager::new()?;
            let items = manager.list_registered_sdks()?;
            match run_sdk_selector(&items, std::mem::take(&mut msg))? {
                SdkSelectorAction::Quit => return Ok(()),
                SdkSelectorAction::BrowseLocal { sdk } => {
                    let versions = manager.list_local_sdk_versions(&sdk)?;
                    if versions.is_empty() {
                        msg = format!("{} has no installed versions", sdk);
                        continue;
                    }
                    let action = run_local_selector(&sdk.to_string(), &versions)?;
                    self.execute_action(action, &sdk)?;
                }
                SdkSelectorAction::BrowseRemote { sdk } => {
                    // 拉取失败（无版本发现源/网络错）→ msg 带回第一层消息区
                    let RemoteVersionResult { items, total_count } =
                        match manager.fetch_remote_version_list(&sdk, self.limit) {
                            Ok(r) => r,
                            Err(e) => {
                                msg = e.to_string();
                                continue;
                            }
                        };
                    let action = run_remote_selector(&sdk.to_string(), &items, self.limit, total_count)?;
                    self.execute_action(action, &sdk)?;
                }
            }
        }
    }

    /// 非 TTY 远程版本文本列表（agent/管道降级，避免 TUI 卡死）
    fn print_remote_text(items: &[RemoteVersionItem]) -> Result<()> {
        for item in items {
            let mark = match item.install_status {
                InstallStatus::Active => "✅",
                InstallStatus::Installed => "📦",
                InstallStatus::NotInstalled => " ",
            };
            println!("{} {}", mark, item.full_version);
        }
        Ok(())
    }
}
