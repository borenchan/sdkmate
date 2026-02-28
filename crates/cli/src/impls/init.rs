use std::env;
use anyhow::{Context, Result};
use clap::Parser;
use crossterm::style::Stylize;
use crate::CommandHandler;

/// init sdkm
pub fn init_sdkm() -> Result<()> {
    println!("initializing sdkm...");
    let dir = env::current_dir()?;
    println!("sdkm current dir: {}", dir.display().to_string().blue());
    println!("{}", "sdkm initialization is successful".green().bold());
    Ok(())
}

#[derive(Debug,Parser)]
pub struct InitHandler {}

impl CommandHandler for InitHandler{
    fn run(&self) -> Result<()> {
        init_sdkm()
    }
}