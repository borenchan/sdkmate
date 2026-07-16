// 各 SDK 版本发现 + 解析编排(install/list 共用)
//
// 注意:下载 URL 构建(`build_download_url`)已移至 `install/download_url.rs`,
// 本模块只负责"版本发现"(把 HTTP 响应解析为 VersionEntry 列表)+ 解析编排。

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use util::config_helper::{
    ArchStyle, OsStyle, PLACEHOLDER_ARCH, PLACEHOLDER_FEATURE_VERSION, PLACEHOLDER_OS, TemplateRenderer,
    detect_arch_with, detect_os_with, detect_platform_triple,
};
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::find_builtin_sdk_config;

use super::cache::{VersionSource, fetch_version_data};
use super::fuzzy::fuzzy_match_version_core;
use super::truncate;

// ─── 数据结构 ────────────────────────────────────────────────────

/// 解析后的版本结果
pub struct ResolvedVersion {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub release_tag: Option<String>,
    /// 直接下载 URL(某些数据源如 uv metadata 已包含完整 URL)
    pub download_url: Option<String>,
    /// 是否为模糊匹配的结果(需要交互确认)
    pub fuzzy_matched: bool,
}

/// 通用版本条目 — 所有 SDK 解析器统一输出此格式
pub struct VersionEntry {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub release_tag: Option<String>,
    pub download_url: Option<String>,
}

// ─── install 侧薄封装:基于核心匹配结果回查 VersionEntry ─────────

/// install 侧薄封装:基于核心匹配结果,回查 VersionEntry 填充附属字段
///
/// 复用 `fuzzy_match_version_core` 的匹配算法;失败时传播核心的"did you mean"错误。
pub fn fuzzy_match_version(entries: &[VersionEntry], version_input: &str) -> Result<ResolvedVersion> {
    let versions: Vec<String> = entries.iter().map(|e| e.full_version.clone()).collect();
    let matched = fuzzy_match_version_core(&versions, version_input)?;

    // 用匹配到的完整版本号回查 entry,填充附属字段
    let entry = entries.iter().find(|e| e.full_version == matched.full_version);
    // 模糊命中时 feature_version 取用户输入(如 "3.12");精确命中时取 entry 自带值
    let feature_version = if matched.fuzzy_matched {
        Some(version_input.to_string())
    } else {
        entry.and_then(|e| e.feature_version.clone())
    };

    Ok(ResolvedVersion {
        full_version: matched.full_version,
        feature_version,
        release_tag: entry.and_then(|e| e.release_tag.clone()),
        download_url: entry.and_then(|e| e.download_url.clone()),
        fuzzy_matched: matched.fuzzy_matched,
    })
}

// ─── 版本发现 trait(VersionDiscovery)──────────────────────────

/// 各 SDK 的版本发现策略:将 HTTP 响应解析为 VersionEntry 列表
pub trait VersionDiscovery: Send + Sync {
    /// SDK 特有:将 HTTP 响应解析为 VersionEntry 列表
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>>;
}

pub fn get_version_discovery(sdk: &Sdk) -> Box<dyn VersionDiscovery> {
    match sdk {
        Sdk::Built(BuiltinSdk::Java) => Box::new(JavaDiscovery),
        Sdk::Built(BuiltinSdk::Node) => Box::new(NodeDiscovery),
        Sdk::Built(BuiltinSdk::Python) => Box::new(PythonDiscovery),
        Sdk::Built(BuiltinSdk::Maven) => Box::new(MavenDiscovery),
        Sdk::Custom(_) => Box::new(ConfigBasedDiscovery),
    }
}

// ─── 通用 resolve 入口 ──────────────────────────────────────────

/// 通用版本解析入口:主备切换 + 缓存 + 模糊匹配
pub async fn resolve_sdk_version(
    client: &Client,
    discovery: &dyn VersionDiscovery,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    version_input: &str,
    headers: Option<HashMap<String, String>>,
    cache_ttl_secs: u64,
) -> Result<ResolvedVersion> {
    let body = fetch_version_data(client, source, cache_key, sdk_label, headers, cache_ttl_secs).await?;
    let entries = discovery.parse_version_data(&body)?;
    fuzzy_match_version(&entries, version_input)
}

// ─── Java 版本发现 ──────────────────────────────────────────────

struct JavaDiscovery;

impl VersionDiscovery for JavaDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // Java 的 version_list_url 返回 available_releases(只有 major 版本号列表)
        // 需要二次查询 assets API 获取精确 semver — 此处只解析第一步
        let data: AdoptiumReleases =
            serde_json::from_str(body).context("[Java version resolve] failed to parse Adoptium releases data")?;
        // Java 的第一步只验证 major 版本是否可用,精确 semver 需要额外逻辑
        // 这里返回 major 版本号作为 entry,后续由 resolve_java_version 处理两步逻辑
        let entries: Vec<VersionEntry> = data
            .available_releases
            .iter()
            .map(|v| VersionEntry {
                full_version: v.to_string(),
                feature_version: Some(v.to_string()),
                release_tag: None,
                download_url: None,
            })
            .collect();
        Ok(entries)
    }
}

/// Java 的两步解析逻辑(available_releases → assets API)
/// 因为 Java 需要两步查询,不能完全套用通用流程,保留独立实现
pub async fn resolve_java_version(client: &Client, version_input: &str) -> Result<ResolvedVersion> {
    let config =
        find_builtin_sdk_config(&BuiltinSdk::Java).context("[Java version resolve] no builtin SDK config for Java")?;

    // 第一步:从 Adoptium 解析可用主版本号
    let resp = client
        .get(config.version_url)
        .send()
        .await
        .context("[Java version resolve] failed to query Adoptium releases API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "[Java version resolve] Adoptium releases API returned HTTP {}, response: {}",
            status,
            truncate(&body, 200)
        );
    }

    let data: AdoptiumReleases = resp
        .json()
        .await
        .context("[Java version resolve] failed to parse Adoptium releases data")?;

    let input_num: i32 = version_input.parse().context(format!(
        "[Java version resolve] '{}' is not a valid Java major version, use an integer like 21/17/11",
        version_input
    ))?;

    if !data.available_releases.contains(&input_num) {
        bail!(
            "[Java version resolve] Java {} not available in Adoptium, available: {:?}",
            version_input,
            data.available_releases
        );
    }

    // 第二步:通过 Adoptium assets API 解析精确 semver
    let assets_url = TemplateRenderer::new()
        .var(PLACEHOLDER_FEATURE_VERSION, version_input)
        .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Adoptium))
        .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Adoptium))
        .render(
            config
                .assets_url
                .context("[Java version resolve] assets_url not configured in builtin SDK config")?,
        )?;

    let resp = client
        .get(&assets_url)
        .send()
        .await
        .context("[Java version resolve] failed to query Adoptium assets API for exact version")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "[Java version resolve] Adoptium assets API returned HTTP {}, response: {}",
            status,
            truncate(&body, 200)
        );
    }

    let assets: Vec<AdoptiumAsset> = resp
        .json()
        .await
        .context("[Java version resolve] failed to parse Adoptium assets data")?;

    if assets.is_empty() {
        // 该版本在此 os/arch 无 Adoptium 包（如 jdk8 macOS 仅 x64，aarch64 无包）
        bail!(
            "[Java version resolve] no JDK release for Java {} on {} {} — Adoptium doesn't ship \
             this version for this platform/arch (e.g. JDK 8 is x64-only on macOS); try another \
             version like 17 or 21",
            input_num,
            detect_os_with(OsStyle::Adoptium),
            detect_arch_with(ArchStyle::Adoptium)
        );
    }

    let full_version = assets[0]
        .version
        .semver
        .split('+')
        .next()
        .unwrap_or(&assets[0].version.semver)
        .to_string();

    Ok(ResolvedVersion {
        full_version,
        feature_version: Some(version_input.to_string()),
        release_tag: None,
        download_url: None,
        fuzzy_matched: false,
    })
}

#[derive(Debug, Deserialize)]
pub struct AdoptiumReleases {
    pub available_releases: Vec<i32>,
    #[allow(dead_code)]
    pub most_recent_lts_version: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    version: AdoptiumVersion,
}
#[derive(Debug, Deserialize)]
struct AdoptiumVersion {
    semver: String,
}

// ─── Node 版本发现 ──────────────────────────────────────────────

struct NodeDiscovery;

impl VersionDiscovery for NodeDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        let versions: Vec<NodeVersion> =
            serde_json::from_str(body).context("[Node version lookup] failed to parse nodejs.org version data")?;

        // 转换为 VersionEntry,保留 LTS 标记用于优先选择
        let entries: Vec<VersionEntry> = versions
            .iter()
            .filter_map(|v| {
                let ver = v.version.trim_start_matches('v').to_string();
                let fv = ver.split('.').next().map(|s| s.to_string());
                Some(VersionEntry {
                    full_version: ver,
                    feature_version: fv,
                    release_tag: None,
                    download_url: None,
                })
            })
            .collect();
        Ok(entries)
    }
}

#[derive(Debug, Deserialize)]
struct NodeVersion {
    version: String,
    #[allow(dead_code)]
    lts: NodeLts,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NodeLts {
    Bool(bool),
    String(String),
}

// ─── Python 版本发现 ────────────────────────────────────────────

struct PythonDiscovery;

impl VersionDiscovery for PythonDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // 先尝试 uv download-metadata.json 格式(弹性解析,单条失败不影响整体)
        if let Ok(entries) = parse_uv_metadata(body) {
            if !entries.is_empty() {
                return Ok(entries);
            }
        }
        // 回退解析 GitHub API 格式(备源)
        parse_github_releases(body)
    }
}

/// 解析 uv download-metadata.json 格式(主源)
/// 使用 serde_json::Value 弹性解析:单条字段不匹配时跳过而非整体失败
fn parse_uv_metadata(body: &str) -> Result<Vec<VersionEntry>> {
    // 先弹性解析为 HashMap<String, Value>,不依赖严格结构体
    let metadata: HashMap<String, serde_json::Value> =
        serde_json::from_str(body).context("[Python version sniff] response is not valid JSON")?;

    let platform_triple = detect_platform_triple()?;
    let target = map_uv_target(&platform_triple)?;

    let mut entries: Vec<VersionEntry> = Vec::new();

    for (_key, value) in &metadata {
        // 只处理 cpython stable 标准版本
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "cpython" {
            continue;
        }

        // variant 必须为 null 或不存在
        if let Some(variant) = value.get("variant") {
            if !variant.is_null() {
                continue;
            }
        }

        // prerelease 必须为空字符串或不存在
        let prerelease = value.get("prerelease").and_then(|v| v.as_str()).unwrap_or("");
        if !prerelease.is_empty() {
            continue;
        }

        // 匹配当前平台
        let os = value.get("os").and_then(|v| v.as_str()).unwrap_or("");
        let libc = value.get("libc").and_then(|v| v.as_str()).unwrap_or("");
        let arch_family = value
            .get("arch")
            .and_then(|v| v.get("family"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if os != target.os || arch_family != target.arch || libc != target.libc {
            continue;
        }

        // 提取版本信息(用默认值避免字段缺失导致失败)
        let major = value.get("major").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let minor = value.get("minor").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let patch = value.get("patch").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let build = value.get("build").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if major == 0 || url.is_empty() {
            continue;
        }

        let full = format!("{}.{}.{}", major, minor, patch);
        entries.push(VersionEntry {
            full_version: full,
            feature_version: Some(major.to_string()),
            release_tag: Some(build),
            download_url: Some(url),
        });
    }

    // 按版本号从高到低排列(模糊匹配取第一个 = 最新)
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });

    Ok(entries)
}

/// uv metadata 的目标平台映射
struct UvTarget {
    os: &'static str,
    arch: &'static str,
    libc: &'static str,
}

/// Rust target triple → uv metadata os+arch+libc 映射
fn map_uv_target(platform_triple: &str) -> Result<UvTarget> {
    match platform_triple {
        "x86_64-pc-windows-msvc" => Ok(UvTarget {
            os: "windows",
            arch: "x86_64",
            libc: "none",
        }),
        "aarch64-pc-windows-msvc" => Ok(UvTarget {
            os: "windows",
            arch: "aarch64",
            libc: "none",
        }),
        "i686-pc-windows-msvc" => Ok(UvTarget {
            os: "windows",
            arch: "i686",
            libc: "none",
        }),
        "x86_64-apple-darwin" => Ok(UvTarget {
            os: "darwin",
            arch: "x86_64",
            libc: "none",
        }),
        "aarch64-apple-darwin" => Ok(UvTarget {
            os: "darwin",
            arch: "aarch64",
            libc: "none",
        }),
        "x86_64-unknown-linux-gnu" => Ok(UvTarget {
            os: "linux",
            arch: "x86_64",
            libc: "gnu",
        }),
        "aarch64-unknown-linux-gnu" => Ok(UvTarget {
            os: "linux",
            arch: "aarch64",
            libc: "gnu",
        }),
        "x86_64-unknown-linux-musl" => Ok(UvTarget {
            os: "linux",
            arch: "x86_64",
            libc: "musl",
        }),
        "aarch64-unknown-linux-musl" => Ok(UvTarget {
            os: "linux",
            arch: "aarch64",
            libc: "musl",
        }),
        _ => bail!(
            "[Python version sniff] unsupported platform '{}' for uv metadata",
            platform_triple
        ),
    }
}

/// 解析 GitHub Releases API 格式(备源)
fn parse_github_releases(body: &str) -> Result<Vec<VersionEntry>> {
    let releases: Vec<GitHubRelease> =
        serde_json::from_str(body).context("[Python version sniff] failed to parse GitHub releases data")?;

    let platform = detect_platform_triple()?;
    let suffix = format!("{}-install_only.tar.gz", platform);

    let mut entries: Vec<VersionEntry> = Vec::new();
    for release in &releases {
        for asset in &release.assets {
            if !asset.name.ends_with(&suffix) {
                continue;
            }
            let version = extract_python_version(&asset.name);
            if let Some(v) = version {
                let fv = v.split('.').next().map(|s| s.to_string());
                entries.push(VersionEntry {
                    full_version: v,
                    feature_version: fv,
                    release_tag: Some(release.tag_name.clone()),
                    download_url: None, // GitHub API 不提供直链,需模板渲染
                });
            }
        }
    }

    // 按版本号从高到低排列
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });

    Ok(entries)
}

fn extract_python_version(name: &str) -> Option<String> {
    let after = name.strip_prefix("cpython-")?;
    Some(after.split('+').next().unwrap_or(after).to_string())
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    #[allow(dead_code)]
    size: u64,
}

// ─── Maven 版本发现 ─────────────────────────────────────────────

struct MavenDiscovery;

impl VersionDiscovery for MavenDiscovery {
    fn parse_version_data(&self, _body: &str) -> Result<Vec<VersionEntry>> {
        // Maven 无远程版本发现,parse 不使用
        Ok(Vec::new())
    }
}

// ─── ConfigBased 版本发现(自定义 SDK)──────────────────────────

struct ConfigBasedDiscovery;

impl VersionDiscovery for ConfigBasedDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // 自定义 SDK 尝试自动解析常见 JSON 版本格式:
        // 1. 扁平字符串数组:["3.12.8", "3.12.7", "3.11.12"]
        // 2. 对象数组含 version 字段:[{"version": "3.12.8"}, ...]
        if let Ok(entries) = parse_flat_version_array(body) {
            return Ok(entries);
        }
        if let Ok(entries) = parse_version_object_array(body) {
            return Ok(entries);
        }
        bail!(
            "[Custom SDK version validate] failed to auto-parse version data from configured version_url. \
               Supported formats: flat string array or array of objects with 'version' field"
        );
    }
}

/// 解析扁平版本字符串数组
fn parse_flat_version_array(body: &str) -> Result<Vec<VersionEntry>> {
    let versions: Vec<String> = serde_json::from_str(body).context("not a flat string array")?;
    let mut entries: Vec<VersionEntry> = versions
        .iter()
        .filter_map(|v| {
            let fv = v.split('.').next().map(|s| s.to_string());
            Some(VersionEntry {
                full_version: v.clone(),
                feature_version: fv,
                release_tag: None,
                download_url: None,
            })
        })
        .collect();
    // 按版本号从高到低
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
    Ok(entries)
}

/// 解析对象数组含 version 字段
fn parse_version_object_array(body: &str) -> Result<Vec<VersionEntry>> {
    let items: Vec<GenericVersionObject> =
        serde_json::from_str(body).context("not an object array with version field")?;
    let mut entries: Vec<VersionEntry> = items
        .iter()
        .filter_map(|item| {
            let fv = item.version.split('.').next().map(|s| s.to_string());
            Some(VersionEntry {
                full_version: item.version.clone(),
                feature_version: fv,
                release_tag: None,
                download_url: None,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
    Ok(entries)
}

#[derive(Debug, Deserialize)]
struct GenericVersionObject {
    version: String,
}
