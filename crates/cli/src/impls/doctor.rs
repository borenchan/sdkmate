use crate::CommandHandler;
use crate::terminal_sink::manager;
use anyhow::Result;
use clap::Parser;

/// `sdkm doctor`：打印诊断报告，供用户提 issue 时粘贴
#[derive(Debug, Parser)]
pub struct DoctorHandler;

impl CommandHandler for DoctorHandler {
    fn run(&self) -> Result<()> {
        let manager = manager()?;
        sdkcore::doctor::show_doctor_report(&manager)
    }
}
