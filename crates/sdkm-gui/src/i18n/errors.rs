//! core 错误文案的 GUI 层中文映射（渐进覆盖，不碰 core）
//!
//! 边界：core 的错误/事件文案是英文（分层约定：文案归 core），GUI 展示端在此对
//! 常见错误模式做翻译，未命中的原样展示。CoreEvent 文案同理经 [`translate`] 过滤。
//!
//! 完整方案是 core 错误码化（结构化错误类型 + 前端查表），属大改造，需单独决策；
//! 本表只做 substring 模式匹配，覆盖高频错误即可。

use crate::prefs::GuiLang;

/// 错误模式表：（英文模式串，中文，英文）
const PATTERNS: &[(&str, &str, &str)] = &[
    // 网络
    (
        "proxy",
        "代理连接失败，请检查代理设置",
        "Proxy connection failed; check proxy settings",
    ),
    ("timed out", "连接超时，请检查网络", "Connection timed out; check your network"),
    ("timeout", "连接超时，请检查网络", "Connection timed out; check your network"),
    (
        "dns error",
        "域名解析失败，请检查网络",
        "DNS resolution failed; check your network",
    ),
    (
        "connection refused",
        "连接被拒绝，服务可能不可用",
        "Connection refused; service may be unavailable",
    ),
    (
        "certificate",
        "证书校验失败，可临时关闭 SSL 校验重试",
        "Certificate verification failed; try disabling SSL verify",
    ),
    // 权限/系统
    (
        "os error 5",
        "权限不足，请以管理员身份运行",
        "Access denied; run as administrator",
    ),
    (
        "os error 1314",
        "创建符号链接失败，需要管理员权限或开发者模式",
        "Symlink creation failed; requires admin or developer mode",
    ),
    (
        "access is denied",
        "权限不足，请以管理员身份运行",
        "Access denied; run as administrator",
    ),
    // 状态
    (
        "not initialized",
        "sdkm 尚未初始化，请先执行初始化",
        "sdkm is not initialized; run init first",
    ),
    ("not found", "未找到目标资源", "Target not found"),
    ("already installed", "该版本已安装", "Version already installed"),
    (
        "rate limit",
        "GitHub API 限流，可配置 Token 提升配额",
        "GitHub API rate limited; configure a token to raise the quota",
    ),
];

/// 尝试翻译 core 错误文案；未命中模式时返回原文（英文原样展示）
pub fn translate(msg: &str, lang: GuiLang) -> String {
    let lower = msg.to_lowercase();
    for (pattern, zh, en) in PATTERNS {
        if lower.contains(pattern) {
            return match lang {
                GuiLang::Zh => (*zh).to_string(),
                GuiLang::En => (*en).to_string(),
            };
        }
    }
    msg.to_string()
}
