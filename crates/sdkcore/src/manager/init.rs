use anyhow::Result;
use crossterm::style::Stylize;
use std::env;

/// init sdkm
pub fn init_sdkm() -> Result<()> {
    println!("initializing sdkm...");
    let dir = env::current_dir()?;
    //create sdk store root dir
    println!("sdkm current dir: {}", dir.display().to_string().blue());
    println!("{}", "sdkm initialization is successful".green().bold());
    Ok(())
}
