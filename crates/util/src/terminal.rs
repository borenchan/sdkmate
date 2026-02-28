use std::path::Display;

use crossterm::style::Stylize;

pub fn success(message: &str) {
    println!("{}", message.green().bold().italic());
}
pub fn info_success(prefix: &str, message: &str) {
    println!("{} {}", prefix.bold(), message.green().bold().italic());
}

pub fn info(message: &str) {
    println!("{}", message.bold());
}

pub fn error(message: &str) {
    println!("{}", message.red().bold().italic());
}
