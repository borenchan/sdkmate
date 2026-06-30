//! 版本解析公共模块(install / switch / list 共用)
//!
//! 职责:
//! - [`cache`]:版本数据缓存 + 网络获取(主备切换 + 重试 + 缓存兜底)
//! - [`fuzzy`]:版本字符串模糊匹配 + 相近版本建议
//! - [`discovery`]:各 SDK 版本发现(parse_version_data)+ 解析编排(resolve_*)
//!
//! 下载 URL 构建不属于"版本解析",见 `install::download_url`。

pub mod cache;
pub mod discovery;
pub mod fuzzy;

pub use cache::{VersionSource, fetch_version_data};
pub use discovery::{
    ResolvedVersion, VersionDiscovery, VersionEntry, fuzzy_match_version, get_version_discovery, resolve_java_version,
    resolve_sdk_version,
};
pub use fuzzy::{FuzzyMatch, fuzzy_match_version_core, suggest_similar_version};

/// 截断字符串到指定长度(缓存重试日志与 Java 解析错误信息共用)
pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}
