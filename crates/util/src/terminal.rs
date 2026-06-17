use anyhow::Result;
use crossterm::style::Stylize;
use std::io::{self, Write};

// ── 统一调色板 ──────────────────────────────────────────────
//
//  角色      | 颜色         | 前缀   | 场景
//  ─────────|──────────────|────────|───────────────────────
//  success   | Green  bold  | ✅     | 操作成功完成
//  info      | Blue   bold  | ℹ️     | 主要状态/引导信息
//  warning   | Yellow bold  | ⚠️     | 需要注意/重试
//  error     | Red    bold  | 🦀     | 操作失败
//  detail    | DarkGray     | 3空格  | 辅助信息(URL/路径/清理)
//  step      | Magenta bold | 📋     | 多步骤阶段标记
//  divider   | DarkGray     | ────── | 分隔线
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

/// 辅助信息：URL、路径、清理等次要输出（暗灰缩进，视觉层级低于 info）
pub fn detail(message: &str) {
    println!("   {}", message.dark_grey());
}

/// 多步骤阶段标记：流程中的阶段性提示
pub fn step(label: &str, message: &str) {
    println!("📋 {}: {}", label.magenta().bold(), message);
}

/// 分隔线：列表或段落之间的视觉分隔
pub fn divider() {
    println!("{}", "─".repeat(50).dark_grey());
}

/// 前缀+成功组合输出
pub fn info_success(prefix: &str, message: &str) {
    println!("{} {}", prefix.blue().bold(), message.green().bold());
}

/// 终端交互确认
pub fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} {}", prompt.dark_blue(), "[yes/No]".dark_blue());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}
