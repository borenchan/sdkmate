use anyhow::{Context, Result};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, ContentStyle, Print, SetBackgroundColor, SetForegroundColor, SetStyle, Stylize},
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use sdkcore::list::{InstallStatus, RegisteredSdkItem, RemoteVersionItem, SdkVersionItem};
use sdkcore::size_cache::SizeCache;
use std::io::{self, Write};
use std::iter::once;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;
use util::consts::{DIVIDER, DIVIDER_DOT, SDK_SELECTOR_TIPS, STATUS_ACTIVE, STATUS_INSTALLED, TUI_TIPS};
use util::path::{format_bytes, get_installed_sdks_dir};
use util::sdk::Sdk;
use util::try_bug;

/// Action returned by the selector when user triggers a command
pub enum SelectorAction {
    Quit,
    Install { version: String },
    Switch { version: String },
    Uninstall { version: String },
}

/// Action returned by the first-layer SDK selector
pub enum SdkSelectorAction {
    /// 用户退出(q/Esc/Ctrl+C)
    Quit,
    /// 本地版本 TUI(已安装 SDK 按 Enter)
    BrowseLocal { sdk: Sdk },
    /// 远程版本 TUI(r 键,或未安装 SDK 按 Enter)
    BrowseRemote { sdk: Sdk },
}

// ─── Layout constants ──────────────────────────────────────────────
/// Max items visible at once — fixed window height, scroll for overflow
const MAX_VISIBLE: usize = 10;
/// Desired display width of the status column (✅=2cols + 1space = 3)
const STATUS_COL_WIDTH: usize = 3;

/// Run interactive version selector for local SDK versions
pub fn run_local_selector(sdk_name: &str, versions: &[SdkVersionItem]) -> Result<SelectorAction> {
    if versions.is_empty() {
        return Ok(SelectorAction::Quit);
    }
    // 后台并行算 size：TUI 先用 "…" 渲染、算完逐条回填（缓存命中即返，冷路径 jwalk 并行 + 回写）
    let n = versions.len();
    let live: Arc<Mutex<Vec<Option<u64>>>> = Arc::new(Mutex::new(vec![None; n]));
    let dirs: Vec<PathBuf> = versions.iter().map(|v| v.sdk_dir.clone()).collect();
    let live_bg = Arc::clone(&live);
    // panic=abort 下后台线程绝不能 panic：resolve/save 均无 unwrap，锁用容错取值
    let bg = thread::Builder::new()
        .name("sdkm-size".into())
        .spawn(move || {
            let mut cache = SizeCache::load();
            for (i, dir) in dirs.iter().enumerate() {
                let bytes = cache.resolve(dir);
                if let Ok(mut g) = live_bg.lock() {
                    g[i] = Some(bytes);
                }
            }
            cache.save();
        })
        .ok();

    let action = run_selector_inner(
        sdk_name,
        "Local Versions",
        None,
        None,
        versions.iter().map(|v| v.sdk_version.as_str()).collect(),
        versions.iter().map(|v| v.is_active).collect(),
        versions.iter().map(|_| true).collect(),  // local = always installed
        versions.iter().map(|_| false).collect(), // no "not installed" in local
        Some(live),
    );

    // 等后台算完再退出：确保缓存落盘 + 数据完整（冷路径最多多等一次 walk，热路径瞬时）
    if let Some(handle) = bg {
        let _ = handle.join();
    }
    action
}

/// Run interactive version selector for remote SDK versions
pub fn run_remote_selector(
    sdk_name: &str,
    items: &[RemoteVersionItem],
    _limit: u32,
    total_count: usize,
) -> Result<SelectorAction> {
    if items.is_empty() {
        return Ok(SelectorAction::Quit);
    }
    let source_url = items.first().map(|i| i.source_url.as_str()).unwrap_or("N/A");

    // Build title with total/limit info (replaces the hidden warning line)
    let title = if total_count > items.len() {
        format!("Remote Versions · {} available · showing {}", total_count, items.len())
    } else {
        format!("Remote Versions · {} available", total_count)
    };

    run_selector_inner(
        sdk_name,
        &title,
        Some(source_url),
        Some(total_count),
        items.iter().map(|i| i.full_version.as_str()).collect(),
        items.iter().map(|i| i.install_status == InstallStatus::Active).collect(),
        items
            .iter()
            .map(|i| i.install_status == InstallStatus::Installed || i.install_status == InstallStatus::Active)
            .collect(),
        items.iter().map(|i| i.install_status == InstallStatus::NotInstalled).collect(),
        None,
    )
}

/// 第一层 SDK 选择 TUI：列出所有已注册 SDK（已安装在上、未安装在下）
///
/// - 已安装行：✅(激活) + sdk + current 版本 + total size（后台线程算，`…` 渐进回填）
/// - 未安装行：暗灰
/// - Enter：已安装 → 本地版本 TUI；未安装 → 远程 TUI（无版本发现源由调用方提示）
/// - r：远程 TUI
/// - 底部 msg 区持久显示上一次反馈（与轮播 tip 分离），初始为 `initial_msg`
pub fn run_sdk_selector(items: &[RegisteredSdkItem], initial_msg: String) -> Result<SdkSelectorAction> {
    if items.is_empty() {
        return Ok(SdkSelectorAction::Quit);
    }

    // 已安装在上(字母序)、未安装在下(字母序)；分组后索引 = 行号
    let mut installed: Vec<&RegisteredSdkItem> = items.iter().filter(|i| i.installed).collect();
    let mut missing: Vec<&RegisteredSdkItem> = items.iter().filter(|i| !i.installed).collect();
    let sort_key = |i: &&RegisteredSdkItem| i.name.to_lowercase();
    installed.sort_by_key(sort_key);
    missing.sort_by_key(sort_key);
    let ordered: Vec<&RegisteredSdkItem> = installed.into_iter().chain(missing).collect();
    let total = ordered.len();

    // 行数据：名字 / current 版本串 / 状态标记
    let names: Vec<&str> = ordered.iter().map(|i| i.name.as_str()).collect();
    let currents: Vec<String> = ordered.iter().map(|i| i.current.clone().unwrap_or_default()).collect();
    let marks: Vec<&str> = ordered
        .iter()
        .map(|i| if i.current.is_some() { STATUS_ACTIVE } else { " " })
        .collect();
    let sizes: Vec<Option<u64>> = vec![None; total];

    // 后台并行算已安装 SDK 的 total size（缓存命中即返，冷路径 jwalk 并行 + 回写）
    let sdks_root = get_installed_sdks_dir().ok();
    let store_dirs: Vec<Option<PathBuf>> = ordered
        .iter()
        .map(|i| i.installed.then(|| sdks_root.as_ref().map(|d| d.join(&i.name))).flatten())
        .collect();
    let live: Arc<Mutex<Vec<Option<u64>>>> = Arc::new(Mutex::new(sizes));
    let live_bg = Arc::clone(&live);
    // panic=abort 下后台线程绝不能 panic
    let bg = thread::Builder::new()
        .name("sdkm-sdk-size".into())
        .spawn(move || {
            let mut cache = SizeCache::load();
            for (i, dir) in store_dirs.iter().enumerate() {
                // 未安装行无目录可算：写 0 让 pending 判定收敛（界面显示空 size 串）
                let bytes = match dir {
                    Some(d) => cache.resolve(d),
                    None => 0,
                };
                if let Ok(mut g) = live_bg.lock() {
                    g[i] = Some(bytes);
                }
            }
            cache.save();
        })
        .ok();

    let action = run_sdk_selector_inner(&names, &currents, &marks, &ordered, live, initial_msg);

    // 等后台算完再退出：确保缓存落盘
    if let Some(handle) = bg {
        let _ = handle.join();
    }
    action
}

/// 第一层 SDK 选择 TUI 核心循环
#[allow(clippy::collapsible_if)] // event loop 嵌套结构属合理，与 run_selector_inner 同款
fn run_sdk_selector_inner(
    names: &[&str],
    currents: &[String],
    marks: &[&str],
    ordered: &[&RegisteredSdkItem],
    live_sizes: Arc<Mutex<Vec<Option<u64>>>>,
    initial_msg: String,
) -> Result<SdkSelectorAction> {
    let total = names.len();
    let mut selected = 0;
    let mut scroll_offset = 0;
    let mut tip_idx = 0;
    let visible = MAX_VISIBLE.min(total);
    // 底部持久消息（操作反馈，无随机性；空串 = 不显示）
    let mut msg = initial_msg;

    // 列样式照 ls 概览表格：表头 sdk/current/total 青粗体 + 分列分色（total 末列不 pad，渐进回填只动行尾）
    let mark_w = UnicodeWidthStr::width(STATUS_ACTIVE);
    let name_col_width = names.iter().map(|n| n.width()).max().unwrap_or(0).max("sdk".width());
    let cur_col_width = currents
        .iter()
        .map(|c| c.width())
        .chain(once("N/A".width()))
        .max()
        .unwrap_or(0)
        .max("current".width());
    // 表头缩进 = 数据行前缀宽（缩进2 + 指针位2 + mark + space + 序号2 + ". "）
    let header_pad = 2 + 2 + mark_w + 1 + 2 + 2;

    try_bug!(crossterm::terminal::enable_raw_mode().context("Failed to enable raw mode"));
    let mut stdout = io::stdout();
    try_bug!(
        execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), crossterm::cursor::Hide)
            .context("Failed to setup TUI mode")
    );

    let result = loop {
        if selected < scroll_offset {
            scroll_offset = selected;
        } else if selected >= scroll_offset + visible {
            scroll_offset = selected - visible + 1;
        }

        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

        let bold = ContentStyle::new().attribute(Attribute::Bold);
        let reset = ContentStyle::new();

        // ── Header ──
        let mut row: u16 = 0;
        // 居中标题：相对表格主体（DIVIDER 宽度）居中，非全屏；样式同 ls 概览表格（ℹ️ 带 VS16 实渲染宽 2 补 1）
        let title = "ℹ️  registered sdks";
        let title_w = UnicodeWidthStr::width(title) + 1;
        let left_pad = DIVIDER.chars().count().saturating_sub(title_w) / 2;
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Blue),
            SetStyle(bold),
            Print(format!("{}{}", " ".repeat(left_pad), title)),
            SetStyle(reset),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // 标题下分隔线（2 空格缩进与表头下分隔线对齐，样式同 ls 概览表格）
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {}", DIVIDER)),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // 表头（青粗体，缩进对齐数据列，样式同 ls 概览表格）
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Cyan),
            SetStyle(bold),
            Print(format!(
                "{}{}  {}  {}",
                " ".repeat(header_pad),
                pad_right("sdk", name_col_width),
                pad_right("current", cur_col_width),
                "total"
            )),
            SetStyle(reset),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // 分隔线（表头与数据隔开，样式同 ls 概览表格）
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {}", DIVIDER)),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // 已安装/未安装分界线之前的行数（滚动指示器会占行，分界行号动态算）
        let boundary_installed = ordered.iter().filter(|i| i.installed).count();

        // ── Scroll indicator (above) ──
        if scroll_offset > 0 {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  ↑ {} more above", scroll_offset)),
                SetForegroundColor(Color::Reset)
            )?;
            row += 1;
        }

        // ── Items ──
        let end = (scroll_offset + visible).min(total);
        for i in scroll_offset..end {
            // 已安装组结束后画一条虚点分组线（首次进入视野时；虚点弱于实线，表意"同列表的软分隔"）
            if i == boundary_installed && boundary_installed > 0 {
                queue!(
                    stdout,
                    MoveTo(0, row),
                    Clear(ClearType::CurrentLine),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("  {}", DIVIDER_DOT)),
                    SetForegroundColor(Color::Reset)
                )?;
                row += 1;
            }

            // 数据行配色分层：已安装=亮色调（sdk 白粗/current 亮绿/total 灰），未安装=整体暗灰 + N/A 占位
            let cur_str = if ordered[i].installed {
                currents[i].clone()
            } else {
                "N/A".to_string()
            };
            let size_str = if ordered[i].installed {
                live_size_str(&live_sizes, i)
            } else {
                "N/A".to_string()
            };
            // mark 补空格到固定列宽保证序号对齐（" " 无 VS16 差异，直接按宽补）
            let mark_pad = " ".repeat(mark_w.saturating_sub(marks[i].width()));

            queue!(stdout, MoveTo(0, row), Clear(ClearType::CurrentLine))?;

            if i == selected {
                // 选中行：高亮背景只盖前缀区（指针 + mark + 序号），sdk 列开始提亮（sdk 亮白粗/current 亮绿）
                queue!(
                    stdout,
                    SetBackgroundColor(Color::DarkCyan),
                    SetForegroundColor(Color::White),
                    SetStyle(bold),
                    Print(format!("  ▸ {}{} {:>2}. ", marks[i], mark_pad, i + 1)),
                    SetBackgroundColor(Color::Reset),
                    SetForegroundColor(Color::Reset),
                    SetForegroundColor(Color::White),
                    SetStyle(bold),
                    Print(pad_right(names[i], name_col_width)),
                    SetStyle(reset),
                    SetForegroundColor(Color::Green),
                    Print(format!("  {}", pad_right(&cur_str, cur_col_width))),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("  {}", size_str)),
                    SetForegroundColor(Color::Reset)
                )?;
            } else {
                let installed = ordered[i].installed;
                // 未选中行：已安装用亮色调，未安装整体暗灰（sdk 名淡青在暗灰组里仍可辨）
                queue!(
                    stdout,
                    SetForegroundColor(if installed { Color::Reset } else { Color::DarkGrey }),
                    Print(format!("    {}{} {:>2}. ", marks[i], mark_pad, i + 1)),
                    SetForegroundColor(if installed { Color::White } else { Color::Cyan }),
                    SetStyle(if installed { bold } else { reset }),
                    Print(pad_right(names[i], name_col_width)),
                    SetStyle(reset),
                    SetForegroundColor(if installed { Color::Green } else { Color::DarkGrey }),
                    Print(format!("  {}", pad_right(&cur_str, cur_col_width))),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("  {}", size_str)),
                    SetForegroundColor(Color::Reset)
                )?;
            }
            row += 1;
        }

        // ── Scroll indicator (below) ──
        let below = total - end;
        if below > 0 {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  ↓ {} more below", below)),
                SetForegroundColor(Color::Reset)
            )?;
            row += 1;
        }

        // ── Footer ──
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {}", DIVIDER)),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // Keybindings
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print("  "),
            SetForegroundColor(Color::Green),
            Print("↑↓/jk nav  "),
            SetForegroundColor(Color::Yellow),
            Print("Enter:browse  r:remote  "),
            SetForegroundColor(Color::DarkGrey),
            Print("q/Ctrl+C:quit"),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // Rotating tip（与 msg 分离：随机轮播内容；第一层用专属按键提示）
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  💡 {}", SDK_SELECTOR_TIPS[tip_idx])),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // 持久消息区（操作反馈，最底行 + 亮红醒目；空串跳过）
        if !msg.is_empty() {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::Red),
                SetStyle(bold),
                Print(format!("  ⚑ {}", msg)),
                SetStyle(reset),
                SetForegroundColor(Color::Reset)
            )?;
        }

        try_bug!(stdout.flush().context("Failed to flush stdout"));

        // size 未算完时短轮询尽快回填，算完回退 3s（仅 tip 轮换）
        let pending = live_sizes.lock().ok().is_some_and(|g| g.iter().any(Option::is_none));
        let poll_timeout = if pending {
            Duration::from_millis(120)
        } else {
            Duration::from_secs(3)
        };
        let ev = try_bug!(event::poll(poll_timeout).context("Event poll failed"));
        if ev {
            if let Event::Key(key) = try_bug!(event::read().context("Event read failed")) {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break SdkSelectorAction::Quit;
                }
                let item = ordered[selected];
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected < total - 1 {
                            selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // 已安装 → 本地版本 TUI；未安装 → 远程 TUI（无版本发现源提示后留在原地）
                        if item.installed {
                            break SdkSelectorAction::BrowseLocal { sdk: item.sdk.clone() };
                        } else if item.has_version_url {
                            break SdkSelectorAction::BrowseRemote { sdk: item.sdk.clone() };
                        } else {
                            msg = format!("{} has no version discovery source, cannot browse remotely", item.name);
                        }
                    }
                    KeyCode::Char('r') => {
                        if item.has_version_url {
                            break SdkSelectorAction::BrowseRemote { sdk: item.sdk.clone() };
                        }
                        msg = format!("{} has no version discovery source, cannot browse remotely", item.name);
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        break SdkSelectorAction::Quit;
                    }
                    _ => {}
                }
            }
        }
        tip_idx = (tip_idx + 1) % SDK_SELECTOR_TIPS.len();
    };

    try_bug!(execute!(stdout, crossterm::cursor::Show, LeaveAlternateScreen).context("Failed to restore terminal"));
    try_bug!(crossterm::terminal::disable_raw_mode().context("Failed to disable raw mode"));

    Ok(result)
}

/// Pad a status mark to the desired display column width
fn pad_status(mark: &str) -> String {
    let w = UnicodeWidthStr::width(mark);
    let padding = STATUS_COL_WIDTH.saturating_sub(w);
    format!("{}{}", mark, " ".repeat(padding))
}

/// 左对齐补齐到指定显示列宽
fn pad_right(s: &str, width: usize) -> String {
    let pad = width.saturating_sub(s.width());
    format!("{}{}", s, " ".repeat(pad))
}

/// Core TUI selector loop — fixed-height window with scroll rollover
#[allow(clippy::too_many_arguments, clippy::collapsible_if)] // TUI 渲染参数多 + event loop 嵌套结构属合理
fn run_selector_inner(
    sdk_name: &str,
    title: &str,
    source_url: Option<&str>,
    _total_count: Option<usize>,
    versions: Vec<&str>,
    is_active: Vec<bool>,
    is_installed: Vec<bool>,
    is_not_installed: Vec<bool>,
    live_sizes: Option<Arc<Mutex<Vec<Option<u64>>>>>,
) -> Result<SelectorAction> {
    let total = versions.len();
    let mut selected = find_default_selection(&is_active);
    let mut scroll_offset = 0;
    let mut tip_idx = 0;
    let visible = MAX_VISIBLE.min(total);

    // 版本列宽度：取所有版本字符串的最大显示宽度，左对齐 pad 保证 size 列对齐
    let version_col_width = versions.iter().map(|v| v.width()).max().unwrap_or(0);

    // Enter TUI mode: raw mode + alternate screen + hide cursor
    try_bug!(crossterm::terminal::enable_raw_mode().context("Failed to enable raw mode"));
    let mut stdout = io::stdout();
    try_bug!(
        execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), crossterm::cursor::Hide)
            .context("Failed to setup TUI mode")
    );

    let result = loop {
        // ── Clamp scroll so selected stays inside visible window ──
        if selected < scroll_offset {
            scroll_offset = selected;
        } else if selected >= scroll_offset + visible {
            scroll_offset = selected - visible + 1;
        }

        // ── Render frame ──
        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

        let bold = ContentStyle::new().attribute(Attribute::Bold);
        let reset = ContentStyle::new();

        // ── Header ──
        let mut row: u16 = 0;

        // Title
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Cyan),
            SetStyle(bold),
            Print(format!("  {} · {}", sdk_name, title)),
            SetStyle(reset),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // Source URL (remote only)
        if let Some(url) = source_url {
            let (_, cols) = crossterm::terminal::size().unwrap_or((24, 80));
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  Source: {}", truncate_url(url, cols))),
                SetForegroundColor(Color::Reset)
            )?;
            row += 1;
        }

        // Divider
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {}", DIVIDER)),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // ── Scroll indicator (above items) ──
        let above = scroll_offset;
        if above > 0 {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  ↑ {} more above", above)),
                SetForegroundColor(Color::Reset)
            )?;
            row += 1;
        }

        // ── Items (table layout, fixed visible window) ──
        let end = (scroll_offset + visible).min(total);
        for i in scroll_offset..end {
            // Status column with unicode-aware padding for terminal column alignment
            let status_mark = if is_active[i] {
                STATUS_ACTIVE
            } else if is_installed[i] {
                STATUS_INSTALLED
            } else {
                " "
            };
            let status_col = pad_status(status_mark);

            // 版本列左对齐 pad 到 version_col_width；本地列表追加 size 列（live：算完显值，未算显 "…"）
            let version_padded = pad_right(versions[i], version_col_width);
            let text = match &live_sizes {
                Some(ls) => format!("{}{}  {}", status_col, version_padded, live_size_str(ls, i)),
                None => format!("{}{}", status_col, version_padded),
            };

            queue!(stdout, MoveTo(0, row), Clear(ClearType::CurrentLine))?;

            if i == selected {
                // Highlighted row: pointer + DarkCyan bg
                queue!(
                    stdout,
                    SetBackgroundColor(Color::DarkCyan),
                    SetForegroundColor(Color::White),
                    SetStyle(bold),
                    Print(format!("  ▸ {}", text)),
                    SetStyle(reset),
                    SetBackgroundColor(Color::Reset),
                    SetForegroundColor(Color::Reset)
                )?;
            } else if is_installed[i] {
                // Installed (non-selected): green fg to stand out
                queue!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("    {}", text)),
                    SetForegroundColor(Color::Reset)
                )?;
            } else {
                // Not installed: dim dark grey fg
                queue!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("    {}", text)),
                    SetForegroundColor(Color::Reset)
                )?;
            }
            row += 1;
        }

        // ── Scroll indicator (below items) ──
        let below = total - end;
        if below > 0 {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  ↓ {} more below", below)),
                SetForegroundColor(Color::Reset)
            )?;
            row += 1;
        }

        // ── Footer ──
        // Divider
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  {}", DIVIDER)),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // Keybindings — clarify which keys work on which rows
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print("  "),
            SetForegroundColor(Color::Green),
            Print("↑↓/jk nav  "),
            SetForegroundColor(Color::Yellow),
            Print("i:install(remote)  "),
            SetForegroundColor(Color::DarkGrey),
            Print("Enter/s:switch(installed)  "),
            SetForegroundColor(Color::Red),
            Print("del/d:uninstall(installed)  q/Ctrl+C:quit"),
            SetForegroundColor(Color::Reset)
        )?;
        row += 1;

        // Rotating tip
        queue!(
            stdout,
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  💡 {}", TUI_TIPS[tip_idx])),
            SetForegroundColor(Color::Reset)
        )?;

        // Single flush — everything drawn at once, no partial states
        try_bug!(stdout.flush().context("Failed to flush stdout"));

        // ── Event loop ──
        // size 尚未算完时短轮询（120ms）让回填尽快刷到屏；全部算完回退 3s（仅 tip 轮换）
        let pending = live_sizes
            .as_ref()
            .and_then(|ls| ls.lock().ok())
            .is_some_and(|g| g.iter().any(Option::is_none));
        let poll_timeout = if pending {
            Duration::from_millis(120)
        } else {
            Duration::from_secs(3)
        };
        let ev = try_bug!(event::poll(poll_timeout).context("Event poll failed"));
        if ev {
            if let Event::Key(key) = try_bug!(event::read().context("Event read failed")) {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                // Ctrl+C → quit (raw mode disables SIGINT)
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break SelectorAction::Quit;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected < total - 1 {
                            selected += 1;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('s') => {
                        if is_installed[selected] || is_active[selected] {
                            break SelectorAction::Switch {
                                version: versions[selected].to_string(),
                            };
                        }
                    }
                    KeyCode::Char('i') => {
                        if is_not_installed[selected] {
                            break SelectorAction::Install {
                                version: versions[selected].to_string(),
                            };
                        }
                    }
                    KeyCode::Delete | KeyCode::Char('d') => {
                        // 仅已安装版本可卸载（本地列表全部可卸载；远程仅已安装）
                        if is_installed[selected] || is_active[selected] {
                            break SelectorAction::Uninstall {
                                version: versions[selected].to_string(),
                            };
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        break SelectorAction::Quit;
                    }
                    _ => {}
                }
            }
        }
        tip_idx = (tip_idx + 1) % TUI_TIPS.len();
    };

    // Restore terminal: show cursor + leave alternate screen + disable raw
    try_bug!(execute!(stdout, crossterm::cursor::Show, LeaveAlternateScreen).context("Failed to restore terminal"));
    try_bug!(crossterm::terminal::disable_raw_mode().context("Failed to disable raw mode"));

    Ok(result)
}

fn find_default_selection(is_active: &[bool]) -> usize {
    is_active.iter().position(|a| *a).unwrap_or(0)
}

/// 从 live size 共享状态读第 i 项的展示串：已算→格式化，未算→"…"
///
/// 锁中毒时取内部数据继续（不 panic —— release 下 panic=abort 会终止整个进程）
fn live_size_str(ls: &Mutex<Vec<Option<u64>>>, i: usize) -> String {
    let guard = ls.lock().unwrap_or_else(|e| e.into_inner());
    match guard.get(i).copied().flatten() {
        Some(b) => format_bytes(b),
        None => "…".to_string(),
    }
}

/// Truncate URL to fit terminal width
fn truncate_url(url: &str, max_cols: u16) -> String {
    let max = max_cols as usize - 12; // "  Source: " prefix + margin
    if url.len() <= max {
        url.to_string()
    } else {
        format!("{}...", &url[..max.saturating_sub(3)])
    }
}
