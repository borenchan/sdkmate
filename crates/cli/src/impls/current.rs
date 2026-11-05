use crate::CommandHandler;
use crate::terminal_sink::manager;
use clap::Parser;
use crossterm::style::Stylize;
use sdkcore::current::VersionOrigin;
use util::path::get_sdkm_home;
use util::terminal::{ColumnColor, print_table};
use util::{divider, info, warning};

#[derive(Debug, Parser)]
pub struct CurrentHandler {
    /// SDK name. Built-in: java, node, python, go, maven, ... (see docs); omit to show all active versions
    #[arg(
        value_name = "SDK",
        help = "SDK name. Built-in: java, node, python, go, maven, ... (see docs). Omit to show all active versions"
    )]
    sdk: Option<String>,
}

impl CommandHandler for CurrentHandler {
    fn run(&self) -> anyhow::Result<()> {
        let manager = manager()?;
        if let Some(sdk_name) = &self.sdk {
            let sdk = manager.match_valid_sdk(sdk_name)?;
            match manager.effective_version(&sdk) {
                Some(ev) => println!(
                    "ℹ️  {} {} ({})",
                    sdk.to_string().bold(),
                    ev.version.green(),
                    styled_origin(&ev.origin)
                ),
                None => println!("ℹ️  {} {}", sdk.to_string().bold(), "N/A".green()),
            }
            return Ok(());
        }
        info!("sdkm home: {}", get_sdkm_home()?.display());
        let all = manager.effective_versions_all()?;
        for (name, err) in &all.skipped {
            warning!("skip unregistered SDK directory `{}`: {}", name, err);
        }
        let mut rows: Vec<Vec<String>> = Vec::new();
        for (sdk, ev) in &all.entries {
            // source 列富文本：三档渐进色自带样式，列色 None 原样输出
            let (current, source) = match ev {
                Some(ev) => (ev.version.clone(), styled_origin(&ev.origin)),
                None => ("N/A".to_string(), "N/A".to_string()),
            };
            rows.push(vec![sdk.to_string(), current, source]);
        }
        divider!();
        if rows.is_empty() {
            info!("no active sdks.");
        } else {
            print_table(
                &["sdk", "current", "source"],
                &rows,
                &[ColumnColor::Bold, ColumnColor::Green, ColumnColor::None],
                4,
            );
        }
        divider!();
        Ok(())
    }
}

/// 三档渐进富文本：session 品红斜体 > project 黄斜体（路径暗化）> global 暗灰斜体
fn styled_origin(origin: &VersionOrigin) -> String {
    match origin {
        VersionOrigin::Session => "session".magenta().italic().to_string(),
        VersionOrigin::Project(path) => format!(
            "{} {}",
            "project".yellow().italic(),
            format!("({})", path.display()).yellow().italic().dim()
        ),
        VersionOrigin::Global => "global".dark_grey().italic().to_string(),
    }
}
