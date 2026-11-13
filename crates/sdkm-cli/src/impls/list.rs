use crate::CommandHandler;
use crate::impls::install::install_with_confirm;
use crate::impls::uninstall::uninstall_with_confirm;
use crate::terminal::{is_tty, pad_right};
use crate::terminal_sink::manager;
use crate::tui::{SdkSelectorAction, SelectorAction, run_local_selector, run_remote_selector, run_sdk_selector};
use crate::{divider, info, warning};
use anyhow::{Result, bail};
use clap::Parser;
use crossterm::{
    cursor, execute,
    style::Stylize,
    terminal::{Clear, ClearType},
};
use sdkm_core::list::{InstallStatus, RemoteVersionItem, RemoteVersionResult, SdkVersionItem};
use sdkm_core::manager::SdkManager;
use sdkm_core::size_cache::SizeCache;
use sdkm_util::consts::{DIVIDER, STATUS_ACTIVE};
use sdkm_util::path::format_bytes;
use sdkm_util::sdk::Sdk;
use std::io::{self, IsTerminal, Write};
use std::iter::once;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Parser)]
pub struct ListHandler {
    /// SDK name. Built-in: java, node, python, go, maven, ... (see docs); omit to browse all registered SDKs
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, go, maven, ... (see docs). Omit to browse all registered SDKs (TUI)"
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
        let manager = manager()?;

        if self.limit == 0 {
            bail!("limit must be >= 1");
        }

        if self.remote && self.sdk.is_none() {
            bail!("Please specify an SDK name, e.g.: sdkm list <sdk> -r");
        }

        match (&self.sdk, self.remote) {
            // 无 SDK → TTY 走两层 SDK/版本选择 TUI；非 TTY（管道/agent）打印已安装概览
            (None, false) => {
                if is_tty() {
                    self.run_sdk_browse_tui()
                } else {
                    Self::show_local_overview(&manager)
                }
            }
            // 本地 SDK → TTY 交互式版本选择器；非 TTY 文本列表
            (Some(sdk_name), false) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                let versions = manager.list_local_sdk_versions(&sdk)?;
                // "未装"提示归展示端（core 数据函数静默——它会污染 hook 的 stdout 捕获）
                if versions.is_empty() {
                    info!("SDK '{}' not found in store. Please install a version first.", sdk);
                }
                if is_tty() {
                    let action = run_local_selector(&sdk.to_string(), &versions)?;
                    self.execute_action(action, &sdk)
                } else {
                    Self::show_local_versions(&versions)
                }
            }
            // 远程 SDK → TTY spinner + 交互式版本选择器；非 TTY 文本列表
            (Some(sdk_name), true) => {
                let sdk = manager.match_valid_sdk(sdk_name)?;
                // TTY 走 TUI 拿全量列表（搜索按全量过滤，--limit 不生效）；非 TTY 文本输出按 --limit 截断
                let tty = is_tty();
                let fetch_limit = if tty { u32::MAX } else { self.limit };
                let RemoteVersionResult { items, total_count } =
                    manager.fetch_remote_version_list(&sdk, fetch_limit)?;
                if !tty && total_count > items.len() {
                    warning!(
                        "{} has {} remote versions, showing first {} (use --limit to change)",
                        sdk_name,
                        total_count,
                        self.limit
                    );
                }
                if tty {
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
                let mut manager = manager()?;
                // TUI 选的是精确版本（fuzzy=false 不触发确认）
                install_with_confirm(&mut manager, sdk, &version, true)
            }
            SelectorAction::Switch { version } => {
                let mut manager = manager()?;
                manager.switch_sdk_to_version(sdk, &version)
            }
            SelectorAction::Uninstall { version } => {
                let mut manager = manager()?;
                // TUI 恒走确认（破坏性操作二次确认，与原 core yes=false 语义一致）
                uninstall_with_confirm(&mut manager, sdk, &version, false)
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
            let manager = manager()?;
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
                    // TUI 拿全量列表（搜索按全量过滤）；--limit 只作用于非 TTY 文本输出
                    let RemoteVersionResult { items, total_count } =
                        match manager.fetch_remote_version_list(&sdk, u32::MAX) {
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

    /// 非 TTY 本地概览渲染（数据来自 core `local_overview_rows`，size 经 SizeCache 解析）
    ///
    /// 居中标题 + 分隔线 + 表格（无顶部分割线，底部一条收尾）：
    /// ```text
    ///            ℹ️  installed sdks
    /// ──────────────────────────────────────────────────
    ///        sdk     current  total
    /// ✅  1. java    8.0.492  518.3 MB
    /// ──────────────────────────────────────────────────
    /// ```
    fn show_local_overview(manager: &SdkManager) -> Result<()> {
        let rows = manager.local_overview_rows()?;
        if rows.is_empty() {
            info!("no installed sdks. run `sdkm install <sdk> <version>` to get started.");
            return Ok(());
        }

        // 缓存优先：先查命中（不计算），全命中直接打印；有未命中走冷路径渐进
        let mut cache = SizeCache::load();
        cache.prune();
        let cached: Vec<Option<u64>> = rows.iter().map(|r| cache.cached(&r.dir)).collect();
        let any_miss = cached.iter().any(Option::is_none);

        let final_items: Vec<(String, String, String)> = if !any_miss {
            // 热路径：全命中，直接组最终表（无延迟）
            rows.iter()
                .zip(cached)
                .map(|(r, sz)| (r.sdk_name.clone(), r.current.clone(), format_bytes(sz.unwrap_or(0))))
                .collect()
        } else {
            // 冷路径：先打骨架（size 列 "…" 提示，先展示 sdk/version），算完重绘成最终表
            let tty = io::stdout().is_terminal();
            if tty {
                let skeleton: Vec<(String, String, String)> = rows
                    .iter()
                    .map(|r| (r.sdk_name.clone(), r.current.clone(), "…".to_string()))
                    .collect();
                let printed = Self::print_local_table(&skeleton);
                let _ = io::stdout().flush();
                // 计算（resolve 命中即返，未命中 jwalk 并行 + 回写）
                let sizes: Vec<u64> = rows.iter().map(|r| cache.resolve(&r.dir)).collect();
                // 重绘：光标回表头行，清屏向下，打最终表
                let mut stdout = io::stdout();
                let _ = execute!(stdout, cursor::MoveToPreviousLine(printed), Clear(ClearType::FromCursorDown),);
                let _ = stdout.flush();
                rows.iter()
                    .zip(sizes)
                    .map(|(r, b)| (r.sdk_name.clone(), r.current.clone(), format_bytes(b)))
                    .collect()
            } else {
                // 管道/非 TTY：算完再打（无渐进，避免 ANSI 污染管道）
                let sizes: Vec<u64> = rows.iter().map(|r| cache.resolve(&r.dir)).collect();
                rows.iter()
                    .zip(sizes)
                    .map(|(r, b)| (r.sdk_name.clone(), r.current.clone(), format_bytes(b)))
                    .collect()
            }
        };

        cache.save();
        Self::print_local_table(&final_items);
        divider!();
        Ok(())
    }

    /// 打印本地 SDK 概览表（居中标题 + 分隔线 + 表头 + 数据行），返回已打印的终端行数
    ///
    /// 标题：居中、带 ℹ️ 图标、小写、无冒号、蓝色粗体；标题下一条分隔线隔开表格。
    /// 抽出供 `show_local_overview` 冷路径重绘：先打骨架（size="…"）→ 算完 →
    /// 光标回退 N 行 + 清屏向下 → 再打最终表。返回行数用于光标回退量。
    fn print_local_table(items: &[(String, String, String)]) -> u16 {
        // 居中标题（无顶部 divider，标题下加 divider 与表格分隔）
        let title = "ℹ️  installed sdks";
        // ℹ️ 带 VS16 在多数终端按 emoji 渲染宽 2，unicode-width 仅算 1 → 补 1，否则居中偏右
        let title_w = UnicodeWidthStr::width(title) + 1;
        let left_pad = DIVIDER.chars().count().saturating_sub(title_w) / 2;
        println!("{}{}", " ".repeat(left_pad), title.blue().bold());
        divider!();
        // 列头用 total 而非 size：概览每行是一个 SDK，size 列 = 该 SDK 全部版本总占用（非 current 版本大小），
        // 用 total 避免与 current 版本号紧邻造成的"current 版本的大小"误读
        let headers = ["sdk", "current", "total"];
        let active_w = UnicodeWidthStr::width(STATUS_ACTIVE);
        // 数据行前缀 = `{status} {idx:>2}. `，表头行用同等宽度空格缩进对齐数据列
        let prefix_w = active_w + 1 + 2 + 2;
        // 列宽取表头与数据中的最大显示宽度
        let sdk_w = items
            .iter()
            .map(|(s, _, _)| s.as_str().width())
            .chain(once(headers[0].width()))
            .max()
            .unwrap_or(0);
        let ver_w = items
            .iter()
            .map(|(_, v, _)| v.as_str().width())
            .chain(once(headers[1].width()))
            .max()
            .unwrap_or(0);
        let size_w = items
            .iter()
            .map(|(_, _, z)| z.as_str().width())
            .chain(once(headers[2].width()))
            .max()
            .unwrap_or(0);
        // 表头行（青粗体，缩进对齐数据列起始位置）
        let header_line = [
            pad_right(headers[0], sdk_w),
            pad_right(headers[1], ver_w),
            pad_right(headers[2], size_w),
        ]
        .join("  ");
        println!("{}{}", " ".repeat(prefix_w), header_line.cyan().bold());
        // 数据行：status + 序号 + sdk(白粗) + version(绿) + size(灰)
        for (i, (sdk, cur, size)) in items.iter().enumerate() {
            let status = if cur == "N/A" {
                " ".repeat(active_w)
            } else {
                STATUS_ACTIVE.to_string()
            };
            println!(
                "{} {:>2}. {}  {}  {}",
                status,
                i + 1,
                pad_right(sdk, sdk_w).bold(),
                pad_right(cur, ver_w).green(),
                size.as_str().dark_grey(),
            );
        }
        // 行数 = 1(标题) + 1(分隔线) + 1(表头) + items.len()
        (3 + items.len()) as u16
    }

    /// 非 TTY 本地版本列表（带 ✅ 激活标记，TUI 的回退展示）
    fn show_local_versions(versions: &[SdkVersionItem]) -> Result<()> {
        let mut i = 1;
        let prefix = " ".repeat(UnicodeWidthStr::width(STATUS_ACTIVE));
        versions.iter().for_each(|v| {
            info!(
                "{} {:>2}. {}",
                if v.is_active { STATUS_ACTIVE } else { &prefix },
                i,
                v.sdk_version
            );
            i += 1;
        });
        Ok(())
    }
}
