// ──────────────────────────────────────────────────────
// sdkm doctor——诊断信息汇总，供用户提 issue 时粘贴
// ──────────────────────────────────────────────────────
//
// 分节打印、探测失败不中断（doctor 不该成为 bug 报告入口）；
// 敏感字段（proxy/token）只报是否设置，不打印内容。

use crate::manager::SdkManager;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use util::consts::GITHUB_ISSUES_URL;
use util::error;
use util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};
use util::terminal::{detail, divider, info, os_version, platform_info, success, warning};

/// 打印完整诊断报告，分节降级
pub fn show_doctor_report(manager: &SdkManager) -> Result<()> {
    divider();
    info("sdkm doctor — diagnostic report");
    divider();

    section_version_and_platform();
    section_paths(&manager.config);
    section_config_summary(&manager.config);
    section_installed_sdks(manager);
    section_health_check(&manager.config);

    divider();
    detail("💡 When filing an issue, paste this entire report:");
    detail(GITHUB_ISSUES_URL);
    divider();
    Ok(())
}

/// 版本与平台
fn section_version_and_platform() {
    info("sdkm version");
    detail(env!("CARGO_PKG_VERSION"));
    info("platform");
    detail(&platform_info());
    info("OS version");
    detail(&os_version());
}

/// Home 与路径
fn section_paths(config: &crate::config::SdkmConfig) {
    info("home & paths");
    match get_sdkm_home() {
        Ok(home) => {
            let overridden = env::var("SDKM_HOME").map(|v| !v.is_empty()).unwrap_or(false);
            if overridden {
                warning(&format!("home: {} (overridden by SDKM_HOME)", home.display()));
            } else {
                detail(&format!("home: {}", home.display()));
            }
            match get_installed_sdks_dir() {
                Ok(store) => detail(&format!(
                    "store dir: {} ({})",
                    store.display(),
                    if store.is_dir() { "exists" } else { "missing" }
                )),
                Err(e) => {
                    error!("store dir: resolve failed: {e:#}");
                }
            }
            match config.resolved_symlink_dir() {
                Ok(links) => detail(&format!(
                    "symlink dir: {} ({})",
                    links,
                    if Path::new(&links).is_dir() {
                        "exists"
                    } else {
                        "missing"
                    }
                )),
                Err(e) => {
                    error!("symlink dir: resolve failed: {e:#}");
                }
            }
            match get_sdkm_config_path() {
                Ok(p) => detail(&format!(
                    "config: {} ({})",
                    p.display(),
                    if p.is_file() { "ok" } else { "missing" }
                )),
                Err(e) => {
                    error!("config: resolve failed: {e:#}");
                }
            }
        }
        Err(e) => {
            error!("home: resolve failed: {e:#}");
        }
    }
}

/// 配置摘要
fn section_config_summary(config: &crate::config::SdkmConfig) {
    info("network config");
    let n = &config.network;
    detail(&format!("proxy: {}", mark_set(n.proxy.is_some())));
    detail(&format!("ssl_verify: {}", n.ssl_verify));
    detail(&format!("connect_timeout: {}s", n.connect_timeout));
    detail(&format!("cache_ttl: {}s", n.cache_ttl_secs));
    detail(&format!("github_token: {}", mark_set(n.github_token.is_some())));
}

/// 已安装 SDK 概览（复用 list_registered_sdks）
fn section_installed_sdks(manager: &SdkManager) {
    info("registered SDKs");
    match manager.list_registered_sdks() {
        Ok(items) => {
            for it in &items {
                detail(&format!(
                    "{:<10} installed={} current={}",
                    it.name,
                    if it.installed { "yes" } else { "no " },
                    it.current.as_deref().unwrap_or("-")
                ));
            }
        }
        Err(e) => {
            error!("installed SDKs: list failed: {e:#}");
        }
    }
}

/// 环境健康检查（只读，每项独立 ✅/⚠️/❌）
fn section_health_check(config: &crate::config::SdkmConfig) {
    info("health checks");

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
                    success(&format!("symlink targets: all valid in {}", links));
                } else {
                    error!("symlink targets broken: {}", broken.join("; "));
                }
            }
            Err(_) => warning("symlink dir: not readable yet (run `sdkm init` first)"),
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
            success("PATH: sdkm links entries present");
        } else {
            warning("PATH: no sdkm links entries (run `sdkm switch` to add, or reopen shell)");
        }
    }

    // 3. 各 SDK 配置的 extra_vars 当前环境值（如 JAVA_HOME）
    for s in &config.sdks {
        if s.extra_vars.is_empty() {
            continue;
        }
        for var in s.extra_vars.keys() {
            match env::var(var) {
                Ok(val) => detail(&format!("{}[{}]: {}", s.name, var, val)),
                Err(_) => detail(&format!("{}[{}]: not set in current shell", s.name, var)),
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
        warning(&format!(
            "SDKM_HOME ({}) differs from exe dir ({}) — configs live under SDKM_HOME",
            home.display(),
            exe_dir.display()
        ));
    }
}

/// "set"/"unset" 简写
fn mark_set(set: bool) -> &'static str {
    if set { "set" } else { "unset" }
}
