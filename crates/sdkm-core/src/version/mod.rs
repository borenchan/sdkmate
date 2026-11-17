//! 版本解析公共模块(install / switch / list 共用)
//!
//! 职责:
//! - [`cache`]:版本数据缓存 + 网络获取(主备切换 + 重试 + 缓存兜底)
//! - [`fuzzy`]:版本字符串模糊匹配 + 相近版本建议
//! - [`discovery`]:各 SDK 版本发现(parse_version_data)+ 解析编排(resolve_*)
//!
//! 下载 URL 构建不属于"版本解析",见 `install::download_url`。

pub mod asset_selector;
pub mod cache;
pub mod discovery;
pub mod fuzzy;
pub mod github_releases;

pub use cache::{VersionSource, fetch_version_data};
pub use discovery::{
    ResolvedVersion, VersionDiscovery, VersionEntry, fetch_java_list_majors, fetch_tomcat_entries, fuzzy_match_version,
    get_version_discovery, list_java_versions, resolve_java_version, resolve_sdk_version, resolve_tomcat_version,
};
pub use fuzzy::{
    FuzzyMatch, JavaVendor, format_java_version, fuzzy_match_version_core, fuzzy_match_version_with_vendor,
    parse_java_version_input, suggest_similar_version,
};
pub use github_releases::{ensure_per_page, github_api_headers, is_github_releases_url};

use anyhow::Result;
use sdkm_util::sdk::{BuiltinSdk, Sdk};

/// 本地已装版本匹配入口：Java 走供应商后缀感知匹配，其余 SDK 走通用模糊匹配
pub fn match_installed_version(sdk: &Sdk, versions: &[String], input: &str) -> Result<FuzzyMatch> {
    if matches!(sdk, Sdk::Built(BuiltinSdk::Java)) {
        fuzzy_match_version_with_vendor(versions, input)
    } else {
        fuzzy_match_version_core(versions, input)
    }
}

/// 截断字符串到指定长度(缓存重试日志与 Java 解析错误信息共用)
pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}
