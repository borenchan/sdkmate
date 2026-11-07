// ──────────────────────────────────────────────────────
// sdkm doctor——诊断数据收集，供用户提 issue 时粘贴
// ──────────────────────────────────────────────────────
//
// 数据化：collect_doctor_report 只产出结构化 DoctorReport（零输出），渲染归前端
// （cli 按 level 分流 stdout/stderr）。分节收集、探测失败不中断（doctor 不该成为
// bug 报告入口）；敏感字段（proxy/token）只报是否设置，不打印内容。

use crate::config::SdkmConfig;
use crate::manager::SdkManager;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use util::diagnostics::{os_version, platform_info};
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};

/// 诊断报告数据（渲染归前端；行的先后顺序即展示顺序，节标题也是 Info 行）
pub struct DoctorReport {
    pub lines: Vec<DoctorLine>,
}

/// 单行诊断项
pub struct DoctorLine {
    pub level: DoctorLevel,
    pub text: String,
}

/// 行级别（前端据此选样式/分流 stdout 与 stderr）
#[derive(Clone, Copy)]
pub enum DoctorLevel {
    Info,
    Success,
    Warning,
    Error,
    Detail,
}

/// 收集完整诊断报告（只读，零输出）
pub fn collect_doctor_report(manager: &SdkManager) -> Result<DoctorReport> {
    let mut lines = Vec::new();
    section_version_and_platform(&mut lines);
    section_paths(&manager.config, &mut lines);
    section_config_summary(&manager.config, &mut lines);
    section_installed_sdks(manager, &mut lines);
    section_health_check(&manager.config, &mut lines);
    Ok(DoctorReport { lines })
}

fn line(level: DoctorLevel, text: impl Into<String>) -> DoctorLine {
    DoctorLine {
        level,
        text: text.into(),
    }
}

/// 版本与平台
fn section_version_and_platform(lines: &mut Vec<DoctorLine>) {
    lines.push(line(DoctorLevel::Info, "sdkm version"));
    lines.push(line(DoctorLevel::Detail, env!("CARGO_PKG_VERSION")));
    lines.push(line(DoctorLevel::Info, "platform"));
    lines.push(line(DoctorLevel::Detail, platform_info()));
    lines.push(line(DoctorLevel::Info, "OS version"));
    lines.push(line(DoctorLevel::Detail, os_version()));
}

/// Home 与路径
fn section_paths(config: &SdkmConfig, lines: &mut Vec<DoctorLine>) {
    lines.push(line(DoctorLevel::Info, "home & paths"));
    match get_sdkm_home() {
        Ok(home) => {
            let overridden = env::var("SDKM_HOME").map(|v| !v.is_empty()).unwrap_or(false);
            if overridden {
                lines.push(line(
                    DoctorLevel::Warning,
                    format!("home: {} (overridden by SDKM_HOME)", home.display()),
                ));
            } else {
                lines.push(line(DoctorLevel::Detail, format!("home: {}", home.display())));
            }
            match get_installed_sdks_dir() {
                Ok(store) => lines.push(line(
                    DoctorLevel::Detail,
                    format!(
                        "store dir: {} ({})",
                        store.display(),
                        if store.is_dir() { "exists" } else { "missing" }
                    ),
                )),
                Err(e) => {
                    lines.push(line(DoctorLevel::Error, format!("store dir: resolve failed: {e:#}")));
                }
            }
            match config.resolved_symlink_dir() {
                Ok(links) => lines.push(line(
                    DoctorLevel::Detail,
                    format!(
                        "symlink dir: {} ({})",
                        links,
                        if Path::new(&links).is_dir() {
                            "exists"
                        } else {
                            "missing"
                        }
                    ),
                )),
                Err(e) => {
                    lines.push(line(DoctorLevel::Error, format!("symlink dir: resolve failed: {e:#}")));
                }
            }
            match get_sdkm_config_path() {
                Ok(p) => lines.push(line(
                    DoctorLevel::Detail,
                    format!("config: {} ({})", p.display(), if p.is_file() { "ok" } else { "missing" }),
                )),
                Err(e) => {
                    lines.push(line(DoctorLevel::Error, format!("config: resolve failed: {e:#}")));
                }
            }
        }
        Err(e) => {
            lines.push(line(DoctorLevel::Error, format!("home: resolve failed: {e:#}")));
        }
    }
}

/// 配置摘要
fn section_config_summary(config: &SdkmConfig, lines: &mut Vec<DoctorLine>) {
    let n = &config.network;
    lines.push(line(DoctorLevel::Info, "network config"));
    lines.push(line(DoctorLevel::Detail, format!("proxy: {}", mark_set(n.proxy.is_some()))));
    lines.push(line(DoctorLevel::Detail, format!("ssl_verify: {}", n.ssl_verify)));
    lines.push(line(DoctorLevel::Detail, format!("connect_timeout: {}s", n.connect_timeout)));
    lines.push(line(DoctorLevel::Detail, format!("cache_ttl: {}s", n.cache_ttl_secs)));
    lines.push(line(
        DoctorLevel::Detail,
        format!("github_token: {}", mark_set(n.github_token.is_some())),
    ));
}

/// 已安装 SDK 概览（复用 list_registered_sdks）
fn section_installed_sdks(manager: &SdkManager, lines: &mut Vec<DoctorLine>) {
    lines.push(line(DoctorLevel::Info, "registered SDKs"));
    match manager.list_registered_sdks() {
        Ok(items) => {
            for it in &items {
                lines.push(line(
                    DoctorLevel::Detail,
                    format!(
                        "{:<10} installed={} current={}",
                        it.name,
                        if it.installed { "yes" } else { "no " },
                        it.current.as_deref().unwrap_or("-")
                    ),
                ));
            }
        }
        Err(e) => {
            lines.push(line(DoctorLevel::Error, format!("installed SDKs: list failed: {e:#}")));
        }
    }
}

/// 环境健康检查（只读，每项独立 ✅/⚠️/❌）
fn section_health_check(config: &SdkmConfig, lines: &mut Vec<DoctorLine>) {
    lines.push(line(DoctorLevel::Info, "health checks"));

    // 1. symlink 目录各链接目标有效性
    if let Ok(links) = config.resolved_symlink_dir() {
        match fs::read_dir(&links) {
            Ok(entries) => {
                let mut broken: Vec<String> = vec![];
                for e in entries.flatten() {
                    match fs::read_link(e.path()) {
                        Ok(target) if target.exists() => {}
                        Ok(target) => {
                            broken.push(format!(
                                "{} → {} (target missing)",
                                e.file_name().to_string_lossy(),
                                target.display()
                            ));
                        }
                        Err(_) => broken.push(format!("{} (not a symlink)", e.file_name().to_string_lossy())),
                    }
                }
                if broken.is_empty() {
                    lines.push(line(DoctorLevel::Success, format!("symlink targets: all valid in {links}")));
                } else {
                    lines.push(line(
                        DoctorLevel::Error,
                        format!("symlink targets broken: {}", broken.join("; ")),
                    ));
                }
            }
            Err(_) => lines.push(line(
                DoctorLevel::Warning,
                "symlink dir: not readable yet (run `sdkm init` first)",
            )),
        }
    }

    // 2. PATH 中是否有 <links>/ 前缀条目（switch 注入的是 links/<sdk>/bin 等子目录，不是 links 本体；
    //    分隔符用 MAIN_SEPARATOR 兼容 Windows 反斜杠与 Unix 正斜杠）
    if let Ok(links) = config.resolved_symlink_dir() {
        let links_norm = format!("{}{}", Path::new(&links).to_string_lossy(), std::path::MAIN_SEPARATOR);
        let in_path = env::var("PATH")
            .map(|p| env::split_paths(&p).any(|seg| seg.to_string_lossy().starts_with(&links_norm)))
            .unwrap_or(false);
        if in_path {
            lines.push(line(DoctorLevel::Success, "PATH: sdkm links entries present"));
        } else {
            lines.push(line(
                DoctorLevel::Warning,
                "PATH: no sdkm links entries (run `sdkm switch` to add, or reopen shell)",
            ));
        }
    }

    // 3. 各 SDK 配置的 extra_vars 当前环境值（如 JAVA_HOME）
    for s in &config.sdks {
        if s.extra_vars.is_empty() {
            continue;
        }
        for var in s.extra_vars.keys() {
            match env::var(var) {
                Ok(val) => lines.push(line(DoctorLevel::Detail, format!("{}[{}]: {}", s.name, var, val))),
                Err(_) => lines.push(line(
                    DoctorLevel::Detail,
                    format!("{}[{}]: not set in current shell", s.name, var),
                )),
            }
        }
    }

    // 4. SDKM_HOME 与 exe 目录一致性（便携搬运警告）
    let overridden = env::var("SDKM_HOME").map(|v| !v.is_empty()).unwrap_or(false);
    if overridden
        && let Some(exe_dir) = env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
        && let Ok(home) = get_sdkm_home()
        && home != exe_dir
    {
        lines.push(line(
            DoctorLevel::Warning,
            format!(
                "SDKM_HOME ({}) differs from exe dir ({}) — configs live under SDKM_HOME",
                home.display(),
                exe_dir.display()
            ),
        ));
    }
}

/// "set"/"unset" 简写
fn mark_set(set: bool) -> &'static str {
    if set { "set" } else { "unset" }
}
