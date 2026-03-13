use crate::CommandHandler;
use anyhow::Result;
use clap::Parser;
use sdkcore::manager::init::init_sdkm;

#[derive(Debug,Parser)]
pub struct InitHandler {}

impl CommandHandler for InitHandler{
    fn run(&self) -> Result<()> {
        init_sdkm()
    }
}