use crate::consts::{DIVIDER, GITHUB_ISSUES_URL};
use anyhow::Result;
use crossterm::style::Stylize;
use std::io::{self, Write};
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

pub fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} {}", prompt.dark_blue(), "[yes/No]".dark_blue());
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
        let username = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USERPROFILE").map(|p| p.rsplit('\\').next().unwrap_or("YourName").to_string()))
            .unwrap_or_else(|_| "YourName".to_string());
        format!("C:\\Users\\{}\\sdkm\\", username)
    }

    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/usr/local".to_string());
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
         **Command**: `{command}`\n\
         **Error**: {error_msg}\n\
         **Platform**: {platform}\n\n\
         **Steps to reproduce**:\n\
         1. \n\n\
         **Expected behavior**:\n\n\
         **Additional context**:\n",
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
    let os = std::env::var("OS").unwrap_or_else(|_| {
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
