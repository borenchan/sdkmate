use anyhow::{Context, Result};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, ContentStyle, Print, SetBackgroundColor, SetForegroundColor, SetStyle, Stylize},
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use sdkcore::list::{InstallStatus, RemoteVersionItem, SdkVersionItem};
use std::io::{self, Write};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;
use util::consts::{DIVIDER, STATUS_ACTIVE, STATUS_INSTALLED, TUI_TIPS};
use util::path::format_bytes;
use util::try_bug;

/// Action returned by the selector when user triggers a command
pub enum SelectorAction {
    Quit,
    Install { version: String },
    Switch { version: String },
    Uninstall { version: String },
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
    run_selector_inner(
        sdk_name,
        "Local Versions",
        None,
        None,
        versions.iter().map(|v| v.sdk_version.as_str()).collect(),
        versions.iter().map(|v| v.is_active).collect(),
        versions.iter().map(|_| true).collect(),  // local = always installed
        versions.iter().map(|_| false).collect(), // no "not installed" in local
        Some(versions.iter().map(|v| format_bytes(v.size_bytes)).collect()),
    )
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
    sizes: Option<Vec<String>>,
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

            // 版本列左对齐 pad 到 version_col_width；本地列表追加 size 列
            let version_padded = pad_right(versions[i], version_col_width);
            let text = match &sizes {
                Some(sz) => format!("{}{}  {}", status_col, version_padded, sz[i]),
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
        let ev = try_bug!(event::poll(Duration::from_secs(3)).context("Event poll failed"));
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

/// Truncate URL to fit terminal width
fn truncate_url(url: &str, max_cols: u16) -> String {
    let max = max_cols as usize - 12; // "  Source: " prefix + margin
    if url.len() <= max {
        url.to_string()
    } else {
        format!("{}...", &url[..max.saturating_sub(3)])
    }
}
