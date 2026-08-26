use crate::consts::{DIVIDER, GITHUB_ISSUES_URL};
use anyhow::Result;
use crossterm::style::Stylize;
use std::env;
use std::io::{self, Write};
use std::process::Command;
use unicode_width::UnicodeWidthStr;
use url::Url;

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

pub fn info_success(prefix: &str, message: &str) {
    println!("{} {}", prefix.blue().bold(), message.green().bold());
}

/// 目录树输出：用于展示 sdkm home 的目录结构，比 detail 亮一些
pub fn tree(message: &str) {
    println!("  {}", message.grey());
}

/// banner 输出：无缩进、cyan 色，用于 ASCII art 展示
pub fn banner(message: &str) {
    println!("{}", message.cyan());
}

/// 表格列着色规则：决定该列数据单元格的颜色（表头始终 cyan 粗体）
#[derive(Clone, Copy)]
pub enum ColumnColor {
    /// 默认色（不着色不粗体）
    None,
    /// 默认色粗体——用于 sdk 名（与青色列头区分）
    Bold,
    /// 绿色——用于版本号
    Green,
    /// 暗灰——用于 size 等次要信息
    DarkGrey,
}

/// 按显示宽度左对齐填充到 `width`（右侧补空格），unicode 感知
pub fn pad_right(s: &str, width: usize) -> String {
    let w = s.width();
    format!("{s}{}", " ".repeat(width.saturating_sub(w)))
}

/// 打印左对齐列表格
///
/// 表头 cyan 粗体，数据行按 `colors` 着色（None 则默认色）；统一 2 空格缩进 + 列间 2 空格分隔。
/// 列宽按各列单元格的显示宽度（unicode 感知）取最大值左对齐。
pub fn print_table(headers: &[&str], rows: &[Vec<String>], colors: &[ColumnColor]) {
    // 每列最大显示宽度
    let mut widths: Vec<usize> = headers.iter().map(|h| h.width()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = cell.as_str().width();
            if i >= widths.len() {
                widths.push(w);
            } else if w > widths[i] {
                widths[i] = w;
            }
        }
    }
    // 对齐单元格（纯文本，padding 后再着色，避免 ANSI 序列干扰宽度计算）
    let pad_cell = |c: &str, idx: usize| pad_right(c, widths.get(idx).copied().unwrap_or(0));
    let colorize = |s: String, idx: usize| -> String {
        match colors.get(idx).copied() {
            Some(ColumnColor::Bold) => s.as_str().bold().to_string(),
            Some(ColumnColor::Green) => s.as_str().green().to_string(),
            Some(ColumnColor::DarkGrey) => s.as_str().dark_grey().to_string(),
            _ => s,
        }
    };
    // 表头（cyan 粗体）
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, c)| pad_cell(c, i))
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {}", header_line.cyan().bold());
    // 数据行（按列着色）
    for row in rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, c)| colorize(pad_cell(c.as_str(), i), i))
            .collect::<Vec<_>>()
            .join("  ");
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

/// Generate a suggested sdkm deployment path based on the current platform.
///
/// Windows: `C:\Users\<username>\sdkm\`
/// Unix: `~/.sdkm/` (or `/usr/local/sdkm/` as fallback)
pub fn suggest_sdkm_path() -> String {
    #[cfg(windows)]
    {
        let username = env::var("USERNAME")
            .or_else(|_| env::var("USERPROFILE").map(|p| p.rsplit('\\').next().unwrap_or("YourName").to_string()))
            .unwrap_or_else(|_| "YourName".to_string());
        format!("C:\\Users\\{}\\sdkm\\", username)
    }

    #[cfg(unix)]
    {
        let home = env::var("HOME").unwrap_or_else(|_| "/usr/local".to_string());
        if home.starts_with('/') {
            format!("{}/sdkm/", home)
        } else {
            "/usr/local/sdkm/".to_string()
        }
    }

    #[cfg(not(any(windows, unix)))]
    {
        "~/sdkm/".to_string()
    }
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

/// 构建 GitHub issue URL，带 title + body 参数（公开，供测试调用）
///
/// title: `[issue] <error_summary> — <command>`（问题重点在前，命令作上下文在后）
/// body: 预填命令、完整错误消息、平台信息的模板
pub fn build_bug_report_url(command: &str, error_msg: &str) -> String {
    // 错误消息截断：title 只取前 80 字符避免 URL 过长
    let error_summary = if error_msg.len() > 80 {
        format!("{}...", &error_msg[..77])
    } else {
        error_msg.to_string()
    };
    // title 格式：问题重点放前面，命令作上下文放后面
    let title = format!("[issue] {} — {}", error_summary, command);

    // body 模板：预填关键信息，留出描述区域供用户补充
    let body = format!(
        "**Issue Report**\n\n\
         **Sdkm Version**: {version}\n\
         **Command**: `{command}`\n\
         **Error**: {error_msg}\n\
         **OS**: {os}\n\
         **Platform**: {platform}\n\n\
         **Steps to reproduce**:\n\
         1. \n\n\
         **Expected behavior**:\n\n\
         **Additional context**:\n",
        version = env!("CARGO_PKG_VERSION"),
        os = os_version(),
        command = command,
        error_msg = error_msg,
        platform = platform_info(),
    );

    // 使用 url crate 构建合法 URL（自动 percent-encode）
    let mut url = Url::parse(GITHUB_ISSUES_URL)
        .unwrap_or_else(|_| Url::parse("https://github.com/borenchan/sdkmate/issues/new").unwrap());
    url.query_pairs_mut().append_pair("title", &title).append_pair("body", &body);

    url.to_string()
}

/// 平台信息：操作系统 + 架构
fn platform_info() -> String {
    let os = env::var("OS").unwrap_or_else(|_| {
        if cfg!(windows) {
            "windows".to_string()
        } else {
            "unix".to_string()
        }
    });
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("{} ({})", os, arch)
}

/// 操作系统版本号（Windows build / Unix 内核版本），用于 bug report 区分环境
fn os_version() -> String {
    let output = if cfg!(windows) {
        Command::new("cmd").args(["/c", "ver"]).output()
    } else {
        Command::new("uname").args(["-sr"]).output()
    };
    output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
