use crate::CommandHandler;
use crate::terminal::{detail, divider, info};
use crate::terminal_sink::manager;
use anyhow::Result;
use clap::Parser;
use sdkm_core::doctor::{DoctorLevel, collect_doctor_report};
use sdkm_util::consts::GITHUB_ISSUES_URL;

/// `sdkm doctor`：打印诊断报告，供用户提 issue 时粘贴
#[derive(Debug, Parser)]
pub struct DoctorHandler;

impl CommandHandler for DoctorHandler {
    fn run(&self) -> Result<()> {
        let manager = manager()?;

        // 数据收集（core 零输出）→ 渲染（骨架 divider/标题归 cli，level 分流 stdout/stderr）
        let report = collect_doctor_report(&manager)?;

        divider();
        info("sdkm doctor — diagnostic report");
        divider();
        for l in &report.lines {
            match l.level {
                DoctorLevel::Info => info(&l.text),
                DoctorLevel::Success => crate::terminal::success(&l.text),
                DoctorLevel::Warning => crate::terminal::warning(&l.text),
                DoctorLevel::Error => crate::terminal::error(&l.text),
                DoctorLevel::Detail => detail(&l.text),
            }
        }
        divider();
        detail("💡 When filing an issue, paste this entire report:");
        detail(GITHUB_ISSUES_URL);
        divider();
        Ok(())
    }
}
