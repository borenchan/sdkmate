use anyhow::Result;
use crate::consts::DIVIDER;
use crossterm::style::Stylize;
use std::io::{self, Write};

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
            .or_else(|_| {
                std::env::var("USERPROFILE").map(|p| {
                    p.rsplit('\\')
                        .next()
                        .unwrap_or("YourName")
                        .to_string()
                })
            })
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
