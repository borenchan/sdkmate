//! 平台诊断数据采集（os 版本 / 平台信息 / bug report URL 构造）
//!
//! 纯数据函数，无终端渲染依赖——CLI（doctor/bug report 提示）与 GUI（诊断面板）共用。

use std::env;
use url::Url;

use crate::consts::GITHUB_ISSUES_URL;

// os_version 的 unix 分支用；Windows 走注册表无需子进程
#[cfg(unix)]
use std::process::Command;

/// 平台信息：操作系统 + 架构（doctor 诊断命令复用）
pub fn platform_info() -> String {
    let os = env::var("OS").unwrap_or_else(|_| {
        if cfg!(windows) {
            "windows".to_string()
        } else {
            "unix".to_string()
        }
    });
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("{} ({})", os, arch)
}

/// 操作系统版本号（Windows build / Unix 内核版本），用于 bug report 与 doctor 区分环境
pub fn os_version() -> String {
    // Windows 读注册表（UTF-16 无编码问题；`cmd /c ver` 输出 GBK，中文系统 from_utf8 失败变 unknown）
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_LOCAL_MACHINE;
        let hk = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(cur) = hk.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            && let (Ok(name), Ok(build)) = (
                cur.get_value::<String, _>("ProductName"),
                cur.get_value::<String, _>("CurrentBuildNumber"),
            )
        {
            format!("{name} (build {build})")
        } else {
            "unknown".to_string()
        }
    }
    #[cfg(unix)]
    {
        Command::new("uname")
            .args(["-sr"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(any(windows, unix)))]
    {
        "unknown".to_string()
    }
}

/// 构建 GitHub issue URL，带 title + body 参数（公开，供测试调用）
///
/// title: `[issue] <error_summary> — <command>`（问题重点在前，命令作上下文在后）
/// body: 预填命令、完整错误消息、平台信息的模板
pub fn build_bug_report_url(command: &str, error_msg: &str) -> String {
    // 错误消息截断：title 只取前 80 字符避免 URL 过长
    let error_summary = if error_msg.len() > 80 {
        format!("{}...", &error_msg[..77])
    } else {
        error_msg.to_string()
    };
    // title 格式：问题重点放前面，命令作上下文放后面
    let title = format!("[issue] {} — {}", error_summary, command);

    // body 模板：预填关键信息，留出描述区域供用户补充
    let body = format!(
        "**Issue Report**\n\n\
         **Sdkm Version**: {version}\n\
         **Command**: `{command}`\n\
         **Error**: {error_msg}\n\
         **OS**: {os}\n\
         **Platform**: {platform}\n\n\
         **Steps to reproduce**:\n\
         1. \n\n\
         **Expected behavior**:\n\n\
         **Doctor output** (run `sdkm doctor` and paste here):\n\n\
         **Additional context**:\n",
        version = env!("CARGO_PKG_VERSION"),
        os = os_version(),
        command = command,
        error_msg = error_msg,
        platform = platform_info(),
    );

    // 使用 url crate 构建合法 URL（自动 percent-encode）
    let mut url = Url::parse(GITHUB_ISSUES_URL)
        .unwrap_or_else(|_| Url::parse("https://github.com/borenchan/sdkmate/issues/new").unwrap());
    url.query_pairs_mut().append_pair("title", &title).append_pair("body", &body);

    url.to_string()
}
