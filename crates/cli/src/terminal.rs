//! 终端渲染原语（自 util::terminal 逐字节迁入）：着色输出、表格、确认交互、bug report 提示。
//!
//! 归属 CLI 前端：终端颜色/TTY/光标控制是终端 UI 能力，util 保持 UI 无关
//! （GUI 经 sdkcore→util 不再引入终端渲染库）。

use anyhow::Result;
use crossterm::style::Stylize;
use std::env;
use std::io::{self, IsTerminal, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use util::consts::DIVIDER;
use util::diagnostics::{build_bug_report_url, os_version, platform_info};

// ── Unified color palette ───────────────────────────────────
//
//  Role      | Color         | Prefix | Scene
//  ─────────|──────────────|──────|───────────────────────
//  success   | Green  bold  | ✅    | Operation completed
//  info      | Blue   bold  | ℹ️    | Primary status/guidance
//  warning   | Yellow bold  | ⚠️    | Needs attention/retry
//  error     | Red    bold  | 🦀    | Operation failed
//  detail    | DarkGray     | 3sp   | Secondary info (URLs/paths)
//  step      | Magenta bold | 📋    | Multi-step phase marker
//  divider   | DarkGray     | ──────| Visual separator
//

pub fn success(message: &str) {
    println!("✅ {}", message.green().bold());
}

pub fn warning(message: &str) {
    println!("⚠️  {}", message.yellow().bold());
}

pub fn info(message: &str) {
    println!("ℹ️  {}", message.blue().bold());
}

pub fn error(message: &str) {
    eprintln!("🦀 {}", message.red().bold());
}

pub fn detail(message: &str) {
    println!("   {}", message.dark_grey());
}

pub fn step(label: &str, message: &str) {
    println!("📋 {}: {}", label.magenta().bold(), message);
}

pub fn divider() {
    println!("{}", DIVIDER.dark_grey());
}

/// 目录树输出：用于展示 sdkm home 的目录结构，比 detail 亮一些
pub fn tree(message: &str) {
    println!("  {}", message.grey());
}

/// banner 输出：无缩进、cyan 色，用于 ASCII art 展示
pub fn banner(message: &str) {
    println!("{}", message.cyan());
}

/// 表格单元格着色规则（表头始终 cyan 粗体）
#[derive(Clone, Copy)]
pub enum ColumnColor {
    /// 默认色（不着色不粗体）——用于富文本单元格（自带 ANSI 样式）
    None,
    /// 默认色粗体——用于 sdk 名（与青色列头区分）
    Bold,
    /// 绿色——用于版本号
    Green,
}

/// ANSI 转义序列不计入的显示宽度（富文本单元格参与列宽对齐用）
pub fn display_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_ansi = false;
    for c in s.chars() {
        if in_ansi {
            // SGR 颜色序列以字母（m）终结
            if c.is_ascii_alphabetic() {
                in_ansi = false;
            }
        } else if c == '\x1b' {
            in_ansi = true;
        } else {
            width += c.width().unwrap_or(0);
        }
    }
    width
}

/// 按显示宽度左对齐填充到 `width`（右侧补空格），unicode 感知（ANSI 转义序列不计宽）
pub fn pad_right(s: &str, width: usize) -> String {
    let w = display_width(s);
    format!("{s}{}", " ".repeat(width.saturating_sub(w)))
}

/// 打印左对齐列表格
///
/// 表头 cyan 粗体，数据行按 `colors` 着色（None 则原样输出，供富文本单元格用）；
/// 统一 2 空格缩进，列间 `col_gap` 空格分隔。列宽按各列单元格的显示宽度（unicode 感知）取最大值左对齐。
pub fn print_table(headers: &[&str], rows: &[Vec<String>], colors: &[ColumnColor], col_gap: usize) {
    // 每列最大显示宽度
    let mut widths: Vec<usize> = headers.iter().map(|h| h.width()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell.as_str());
            if i >= widths.len() {
                widths.push(w);
            } else if w > widths[i] {
                widths[i] = w;
            }
        }
    }
    // 对齐单元格（纯文本 padding，富文本串含 ANSI 也不影响宽度）
    let pad_cell = |c: &str, idx: usize| pad_right(c, widths.get(idx).copied().unwrap_or(0));
    let colorize = |s: String, idx: usize| -> String {
        match colors.get(idx).copied() {
            Some(ColumnColor::Bold) => s.as_str().bold().to_string(),
            Some(ColumnColor::Green) => s.as_str().green().to_string(),
            _ => s,
        }
    };
    let gap = " ".repeat(col_gap);
    // 表头（cyan 粗体）
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, c)| pad_cell(c, i))
        .collect::<Vec<_>>()
        .join(&gap);
    println!("  {}", header_line.cyan().bold());
    // 数据行（按列着色）
    for row in rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, c)| colorize(pad_cell(c.as_str(), i), i))
            .collect::<Vec<_>>()
            .join(&gap);
        println!("  {}", line);
    }
}

pub fn prompt_confirm(prompt: &str) -> Result<bool> {
    // 文案整段打印后换行，[yes/No] 提示另起一行，用户紧随输入；长文案可含 \n 多行显示
    println!("{}", prompt.dark_blue());
    print!("{}", "[yes/No] ".dark_blue());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

/// 命令执行失败时，在终端提示 bug report 链接
/// 仅在错误被标记为 BugReport（不可由用户解决）时调用
///
/// URL 带 title + body 参数，用户点击即可跳转预填信息的 issue 页面
/// 关键信息同时用 detail 行单独展示，方便终端无法识别完整 URL 时手动复制
pub fn suggest_bug_report(command: &str, error_msg: &str) {
    detail(DIVIDER);
    detail("💡 This might be a bug in sdkm. If you think so, please report:");
    // URL 加下划线样式，便于终端识别可点击链接
    let url = build_bug_report_url(command, error_msg);
    println!("   {}", url.underlined().dark_grey());
    detail(&format!("sdkm version: {}", env!("CARGO_PKG_VERSION")));
    detail(&format!("OS:          {}", os_version()));
    detail(&format!("Command:  {}", command));
    detail(&format!("Error:    {}", error_msg));
    detail(&format!("Platform: {}", platform_info()));
    detail(DIVIDER);
}

/// 当前会话是否交互终端（stdin+stdout 均 TTY）——TUI 路径与非 TTY 输出路径的分派判据
pub fn is_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}
