use crossterm::style::Stylize;
use anyhow::Result;
use std::io::{self, Write};

pub fn success(message: &str) {
    println!("✅ {}", message.green().bold());
}
pub fn warning(message: &str) {
    println!("⚠️ {}: {}", "warning:".yellow().bold(),message.cyan().bold());
}
pub fn info_success(prefix: &str, message: &str) {
    println!("{} {}", prefix.blue().bold(), message.green().bold());
}

pub fn info(message: &str) {
    println!("{}", message.blue().bold());
}

pub fn error(message: &str) {
    eprintln!("🦀 {}: {}", "error".red().bold() ,message.bold());
}

#[derive(Debug)]
pub enum Level{
    Info,
    Success,
    Warning,
    Error,
}


/// 终端交互确认
pub fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} {}",prompt.dark_blue(), "[yes/No]".dark_blue());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}