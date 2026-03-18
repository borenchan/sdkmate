use crossterm::style::Stylize;

pub fn success(message: &str) {
    println!("{}", message.green().bold());
}
pub fn warning(message: &str) {
    println!("{}: {}", "warning:".yellow().bold(),message.bold());
}
pub fn info_success(prefix: &str, message: &str) {
    println!("{} {}", prefix.blue().bold(), message.green().bold());
}

pub fn info(message: &str) {
    println!("{}", message.blue().bold());
}

pub fn error(message: &str) {
    eprintln!("{}: {}", "error".red().bold() ,message.bold());
}

#[derive(Debug)]
pub enum Level{
    Info,
    Success,
    Warning,
    Error,
}