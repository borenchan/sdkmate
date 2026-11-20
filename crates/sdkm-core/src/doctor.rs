// ──────────────────────────────────────────────────────
// sdkm doctor——诊断数据收集，供用户提 issue 时粘贴
// ──────────────────────────────────────────────────────
//
// 数据化：collect_doctor_report 只产出结构化 DoctorReport（零输出），渲染归前端
// （cli 按 level 分流 stdout/stderr）。分节收集、探测失败不中断（doctor 不该成为
// bug 报告入口）；敏感字段（proxy/token）只报是否设置，不打印内容。

use crate::config::SdkmConfig;
#[cfg(unix)]
use crate::env::env_file_path;
use crate::manager::SdkManager;
use crate::shell::check_shell_hook;
use anyhow::Result;
use sdkm_util::diagnostics::{os_version, platform_info};
use sdkm_util::path::{get_installed_sdks_dir, get_sdkm_config_path, get_sdkm_home};
use std::env;
use std::fs;
use std::path::{MAIN_SEPARATOR, Path};
use std::process::Command;

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
#[derive(Clone, Copy, Debug)]
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
    section_shell_env(&mut lines);
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

/// 键值行：键列按 `width` 左对齐、后接两空格（同一小节的值列对齐，方便扫读）
fn kv_line(key: &str, value: impl std::fmt::Display, width: usize) -> String {
    format!("{key:<width$}  {value}")
}

/// 版本与平台（同一组：sdkm 版本 / 平台 / 系统版本各占一行）
fn section_version_and_platform(lines: &mut Vec<DoctorLine>) {
    lines.push(line(DoctorLevel::Info, "version & platform"));
    lines.push(line(DoctorLevel::Detail, env!("CARGO_PKG_VERSION")));
    lines.push(line(DoctorLevel::Detail, platform_info()));
    lines.push(line(DoctorLevel::Detail, os_version()));
}

/// Home 与路径（键列宽 = symlink）
fn section_paths(config: &SdkmConfig, lines: &mut Vec<DoctorLine>) {
    const W: usize = 7;
    lines.push(line(DoctorLevel::Info, "home & paths"));
    match get_sdkm_home() {
        Ok(home) => {
            let overridden = env::var("SDKM_HOME").map(|v| !v.is_empty()).unwrap_or(false);
            let text = kv_line("home", home.display(), W);
            if overridden {
                lines.push(line(DoctorLevel::Warning, format!("{text} (overridden by SDKM_HOME)")));
            } else {
                lines.push(line(DoctorLevel::Detail, text));
            }
            match get_installed_sdks_dir() {
                Ok(store) => lines.push(line(
                    DoctorLevel::Detail,
                    kv_line(
                        "store",
                        format!("{} ({})", store.display(), if store.is_dir() { "exists" } else { "missing" }),
                        W,
                    ),
                )),
                Err(e) => lines.push(line(DoctorLevel::Error, kv_line("store", format!("resolve failed: {e:#}"), W))),
            }
            match config.resolved_symlink_dir() {
                Ok(links) => lines.push(line(
                    DoctorLevel::Detail,
                    kv_line(
                        "symlink",
                        format!(
                            "{links} ({})",
                            if Path::new(&links).is_dir() {
                                "exists"
                            } else {
                                "missing"
                            }
                        ),
                        W,
                    ),
                )),
                Err(e) => lines.push(line(
                    DoctorLevel::Error,
                    kv_line("symlink", format!("resolve failed: {e:#}"), W),
                )),
            }
            match get_sdkm_config_path() {
                Ok(p) => lines.push(line(
                    DoctorLevel::Detail,
                    kv_line(
                        "config",
                        format!("{} ({})", p.display(), if p.is_file() { "ok" } else { "missing" }),
                        W,
                    ),
                )),
                Err(e) => lines.push(line(DoctorLevel::Error, kv_line("config", format!("resolve failed: {e:#}"), W))),
            }
        }
        Err(e) => lines.push(line(DoctorLevel::Error, kv_line("home", format!("resolve failed: {e:#}"), W))),
    }
}

/// shell 与持久化状态：诊断"环境变量/切换不生效"类问题的入口（键列宽 = env file）
fn section_shell_env(lines: &mut Vec<DoctorLine>) {
    const W: usize = 8;
    lines.push(line(DoctorLevel::Info, "shell & env persistence"));
    match check_shell_hook() {
        Ok(status) => {
            lines.push(line(DoctorLevel::Detail, kv_line("shell", status.shell.display_name(), W)));
            if status.profiles.is_empty() {
                lines.push(line(DoctorLevel::Warning, kv_line("profile", "not found", W)));
            }
            for p in &status.profiles {
                let (level, hint) = if p.registered {
                    (DoctorLevel::Detail, "hook registered")
                } else {
                    (DoctorLevel::Warning, "hook missing, run `sdkm init`")
                };
                lines.push(line(level, kv_line("profile", format!("{} ({hint})", p.path.display()), W)));
            }
        }
        Err(e) => lines.push(line(DoctorLevel::Error, kv_line("shell", format!("detect failed: {e:#}"), W))),
    }
    // 托管 env 文件仅 unix（Windows 走注册表）
    #[cfg(unix)]
    match env_file_path() {
        Ok(p) => lines.push(line(
            DoctorLevel::Detail,
            kv_line(
                "env file",
                format!("{} ({})", p.display(), if p.is_file() { "ok" } else { "missing" }),
                W,
            ),
        )),
        Err(e) => lines.push(line(
            DoctorLevel::Error,
            kv_line("env file", format!("resolve failed: {e:#}"), W),
        )),
    }
}

/// 配置摘要（键列宽 = connect_timeout）
fn section_config_summary(config: &SdkmConfig, lines: &mut Vec<DoctorLine>) {
    const W: usize = 15;
    let n = &config.network;
    lines.push(line(DoctorLevel::Info, "network config"));
    lines.push(line(DoctorLevel::Detail, kv_line("proxy", mark_set(n.proxy.is_some()), W)));
    lines.push(line(DoctorLevel::Detail, kv_line("ssl_verify", n.ssl_verify, W)));
    lines.push(line(
        DoctorLevel::Detail,
        kv_line("connect_timeout", format!("{}s", n.connect_timeout), W),
    ));
    lines.push(line(
        DoctorLevel::Detail,
        kv_line("cache_ttl", format!("{}s", n.cache_ttl_secs), W),
    ));
    lines.push(line(
        DoctorLevel::Detail,
        kv_line("github_token", mark_set(n.github_token.is_some()), W),
    ));
}

/// 已安装 SDK 概览（复用 list_registered_sdks）
///
/// 只列已安装项：未安装的注册项对诊断无价值，几十行 `installed=no` 纯噪音；
/// 注册总数放进标题做上下文（区分"没装"和"没注册"）。
fn section_installed_sdks(manager: &SdkManager, lines: &mut Vec<DoctorLine>) {
    let mut items = match manager.list_registered_sdks() {
        Ok(items) => items,
        Err(e) => {
            lines.push(line(DoctorLevel::Error, format!("installed SDKs: list failed: {e:#}")));
            return;
        }
    };
    let total = items.len();
    items.retain(|it| it.installed);
    lines.push(line(
        DoctorLevel::Info,
        format!("installed SDKs ({} / {} registered)", items.len(), total),
    ));
    if items.is_empty() {
        lines.push(line(DoctorLevel::Detail, "(none)"));
        return;
    }
    // 列宽按最长名字对齐（golangci-lint 等长名字不撑歪）
    let width = items.iter().map(|it| it.name.len()).max().unwrap_or_default();
    for it in &items {
        lines.push(line(
            DoctorLevel::Detail,
            format!("{:<width$}  {}", it.name, it.current.as_deref().unwrap_or("-")),
        ));
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
    //    两侧都过 normalize_path 归一——Windows 上 `/` 与 `\`、盘符大小写差异不算两条不同路径）
    if let Ok(links) = config.resolved_symlink_dir() {
        let prefix = format!("{}{}", normalize_path(Path::new(&links)), MAIN_SEPARATOR);
        let in_path = env::var_os("PATH")
            .is_some_and(|p| env::split_paths(&p).any(|seg| normalize_path(&seg).starts_with(&prefix)));
        if in_path {
            lines.push(line(DoctorLevel::Success, "PATH: sdkm links entries present (current session)"));
        } else {
            lines.push(line(
                DoctorLevel::Warning,
                "PATH: no sdkm links entries in current session (run `sdkm switch` to add, or reopen shell)",
            ));
        }
    }

    // 3. 已安装 SDK 的 extra_vars 当前环境值（如 JAVA_HOME）；未安装的跳过（伪告警）
    let store = get_installed_sdks_dir().ok();
    for s in &config.sdks {
        if s.extra_vars.is_empty() || !store.as_ref().is_some_and(|d| d.join(&s.name).is_dir()) {
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

/// GUI 环境诊断结果（结构化、只读、零输出）
#[derive(Debug, Clone, Default)]
pub struct EnvironmentStatus {
    /// 当前进程/二进制路径
    pub exe_path: String,
    /// 二进制所在目录是否在 PATH
    pub exe_in_path: bool,
    /// `sdkm` 命令是否能执行（PATH 解析 + `--version` 成功）
    pub cli_ok: bool,
    /// `sdkm --version` 报告的版本号（执行失败为 None）
    pub cli_version: Option<String>,
    /// 同目录 CLI 二进制的版本号（用于与 GUI 版本对齐展示；不存在为 None）
    pub sibling_cli_version: Option<String>,
    /// config.toml 路径（展示用）
    pub config_path: String,
    /// config.toml 存在
    pub config_ok: bool,
    /// store 目录路径（展示用）
    pub store_path: String,
    /// store 目录存在
    pub store_ok: bool,
    /// 解析后的符号链接目录
    pub links_dir: String,
    /// 链接目录存在且可读
    pub links_ok: bool,
}

/// 收集 GUI 环境诊断数据（CLI 能否从 PATH 调用、配置/存储/链接状态）
pub fn collect_environment_status(config: &SdkmConfig) -> EnvironmentStatus {
    let exe = env::current_exe().ok();
    let exe_path = exe.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let exe_in_path = exe.as_ref().and_then(|p| p.parent()).map(dir_in_path).unwrap_or(false);
    let (cli_ok, cli_version) = probe_cli();
    let sibling_cli_version = crate::self_update::sibling_cli_version();
    let config_path = get_sdkm_config_path().map(|p| p.display().to_string()).unwrap_or_default();
    let config_ok = get_sdkm_config_path().map(|p| p.is_file()).unwrap_or(false);
    let store_path = get_installed_sdks_dir().map(|p| p.display().to_string()).unwrap_or_default();
    let store_ok = get_installed_sdks_dir().map(|p| p.is_dir()).unwrap_or(false);
    let links_dir = config.resolved_symlink_dir().unwrap_or_default();
    let links_ok = !links_dir.is_empty() && fs::read_dir(&links_dir).is_ok();

    EnvironmentStatus {
        exe_path,
        exe_in_path,
        cli_ok,
        cli_version,
        sibling_cli_version,
        config_path,
        config_ok,
        store_path,
        store_ok,
        links_dir,
        links_ok,
    }
}

/// 目录是否在 PATH 中（Windows 忽略大小写与分隔符差异）
fn dir_in_path(dir: &Path) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    let target = normalize_path(dir);
    env::split_paths(&path).any(|p| normalize_path(&p) == target)
}

fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().trim_end_matches(['/', '\\']).to_string();
    #[cfg(windows)]
    {
        s.replace('/', "\\").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        s
    }
}

/// 尝试执行 PATH 中的 `sdkm --version`（仅诊断，不解析业务输出）
fn probe_cli() -> (bool, Option<String>) {
    match Command::new("sdkm").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let version = text
                .split_whitespace()
                .last()
                .map(|v| v.trim_start_matches('v').to_string())
                .filter(|v| !v.is_empty());
            (true, version)
        }
        _ => (false, None),
    }
}
