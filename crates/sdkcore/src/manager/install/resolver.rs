use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use util::config_helper::{
    ArchStyle, OsStyle, PLACEHOLDER_ARCH, PLACEHOLDER_FEATURE_VERSION, PLACEHOLDER_OS,
    PLACEHOLDER_OS_EXT, PLACEHOLDER_VERSION, PLACEHOLDER_RELEASE_TAG, PLACEHOLDER_PLATFORM,
    TemplateRenderer, detect_arch_with, detect_ext, detect_os_with, detect_platform_triple,
};
use util::path::get_sdkm_home;
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::{find_builtin_sdk_config, SdkSourceConfig};
use util::warning;

// ─── 通用数据结构 ────────────────────────────────────────────────

/// 解析后的版本结果
pub struct ResolvedVersion {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub release_tag: Option<String>,
    /// 直接下载 URL（某些数据源如 uv metadata 已包含完整 URL）
    pub download_url: Option<String>,
    /// 是否为模糊匹配的结果（需要交互确认）
    pub fuzzy_matched: bool,
}

/// 通用版本条目 — 所有 SDK 解析器统一输出此格式
pub struct VersionEntry {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub release_tag: Option<String>,
    pub download_url: Option<String>,
}

/// 版本源配置（主/备）
pub struct VersionSource {
    pub primary_url: String,
    pub secondary_url: Option<String>,
}

impl VersionSource {
    pub fn from_config(config: &SdkSourceConfig) -> Self {
        VersionSource {
            primary_url: config.version_url.to_string(),
            secondary_url: config.version_fallback_url.map(|s: &str| s.to_string()),
        }
    }
}

// ─── 通用缓存 ───────────────────────────────────────────────────

/// 缓存路径：<sdkm_home>/.cache/api/<sdk_name>.json
fn cache_path(sdk_name: &str) -> Result<std::path::PathBuf> {
    let dir = get_sdkm_home()?.join(".cache").join("api");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.json", sdk_name)))
}

fn save_to_cache(sdk_name: &str, body: &str) -> Result<()> {
    fs::write(cache_path(sdk_name)?, body)?;
    Ok(())
}

fn load_from_cache(sdk_name: &str) -> Option<String> {
    cache_path(sdk_name).ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
}

// ─── 通用网络请求：主备切换 + 重试 + 缓存兜底 ──────────────────

/// 最大重试次数（仅对 5xx / 网络错误重试，429/403 不重试）
const MAX_RETRIES: u32 = 3;

/// 主备切换 + 重试 + 缓存兜底的通用版本数据获取
///
/// 流程：主源 → 重试3次 → 切备源 → 重试3次 → 缓存兜底 → bail
pub async fn fetch_version_data(
    client: &Client,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<String> {
    // 尝试主源
    let primary_result = try_url_with_retry(
        client, &source.primary_url, headers.clone(), sdk_label, "primary",
    ).await;

    if let Ok(body) = primary_result {
        let _ = save_to_cache(cache_key, &body);
        return Ok(body);
    }

    // 主源失败，尝试备源
    if let Some(secondary_url) = &source.secondary_url {
        if !secondary_url.is_empty() {
            let secondary_result = try_url_with_retry(
                client, secondary_url, headers.clone(), sdk_label, "secondary",
            ).await;

            if let Ok(body) = secondary_result {
                let _ = save_to_cache(cache_key, &body);
                return Ok(body);
            }
        }
    }

    // 主备都失败，尝试缓存兜底
    if let Some(cached) = load_from_cache(cache_key) {
        warning!("[{}] both primary and secondary sources failed, falling back to local cache (may be outdated)", sdk_label);
        return Ok(cached);
    }

    bail!("[{}] all version sources failed with no local cache available. Check your network or configure a mirror URL", sdk_label);
}

/// 对单个 URL 进行重试（5xx/网络错误重试，429/403 不重试直接返回错误）
async fn try_url_with_retry(
    client: &Client,
    url: &str,
    headers: Option<HashMap<String, String>>,
    sdk_label: &str,
    source_label: &str,
) -> Result<String> {
    for attempt in 0..MAX_RETRIES {
        let mut req = client.get(url);
        if let Some(hdrs) = &headers {
            for (k, v) in hdrs {
                req = req.header(k.as_str(), v.as_str());
            }
        }
        let resp = req.send().await;

        match resp {
            Ok(resp) if resp.status().is_success() => {
                return resp.text().await
                    .context(format!("[{}] failed to read {} source response", sdk_label, source_label));
            }
            Ok(resp) => {
                let status = resp.status();
                let status_code = status.as_u16();

                // 速率限制 → 不重试，直接返回错误（让上层切备源或走缓存）
                if status_code == 403 || status_code == 429 {
                    return Err(anyhow::anyhow!(
                        "[{}] {} source rate-limited (HTTP {})", sdk_label, source_label, status
                    ));
                }

                // 5xx 服务器错误 → 重试
                if status.is_server_error() {
                    if attempt < MAX_RETRIES - 1 {
                        let delay = Duration::from_secs((attempt + 1) as u64);
                        warning!("[{}] {} source server error (HTTP {}), retrying in {}s... (attempt {}/{})",
                                 sdk_label, source_label, status, delay.as_secs(), attempt + 1, MAX_RETRIES);
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "[{}] {} source server error (HTTP {}) after {} retries", sdk_label, source_label, status, MAX_RETRIES
                    ));
                }

                // 其他 HTTP 错误 → 不重试
                return Err(anyhow::anyhow!(
                    "[{}] {} source returned HTTP {}", sdk_label, source_label, status
                ));
            }
            Err(e) => {
                // 网络错误 → 重试
                if attempt < MAX_RETRIES - 1 {
                    let delay = Duration::from_secs((attempt + 1) as u64);
                    warning!("[{}] {} source network error, retrying in {}s... (attempt {}/{}): {}",
                             sdk_label, source_label, delay.as_secs(), attempt + 1, MAX_RETRIES, truncate(&e.to_string(), 100));
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "[{}] {} source network error after {} retries: {}", sdk_label, source_label, MAX_RETRIES, truncate(&e.to_string(), 200)
                ));
            }
        }
    }

    bail!("[{}] {} source all retries exhausted", sdk_label, source_label);
}

// ─── 通用模糊匹配 ────────────────────────────────────────────────

/// 通用模糊版本匹配（以 Node.js 策略为标准）
///
/// - 精确匹配：`"3.12.10"` → 直接命中
/// - 模糊 `"3"` → 最新 stable `3.x.x`
/// - 模糊 `"3.12"` → 最新 `3.12.x`
///
/// 返回 `fuzzy_matched=true` 表示模糊匹配结果，需要交互确认
pub fn fuzzy_match_version(entries: &[VersionEntry], version_input: &str) -> Result<ResolvedVersion> {
    // 精确匹配优先
    if version_input.contains('.') {
        for entry in entries {
            if entry.full_version == version_input {
                return Ok(ResolvedVersion {
                    full_version: entry.full_version.clone(),
                    feature_version: entry.feature_version.clone(),
                    release_tag: entry.release_tag.clone(),
                    download_url: entry.download_url.clone(),
                    fuzzy_matched: false,
                });
            }
        }
        // 精确版本未找到
        bail!("version '{}' not found in available releases", version_input);
    }

    // 模糊匹配：major only → latest patch
    let prefix = format!("{}.", version_input);
    let matching: Vec<&VersionEntry> = entries.iter()
        .filter(|e| e.full_version.starts_with(&prefix))
        .collect();

    if matching.is_empty() {
        bail!("no version matching '{}', try a different major version", version_input);
    }

    // 选最新（列表已按版本从高到低排列）
    let best = matching.first().unwrap();
    Ok(ResolvedVersion {
        full_version: best.full_version.clone(),
        feature_version: Some(version_input.to_string()),
        release_tag: best.release_tag.clone(),
        download_url: best.download_url.clone(),
        fuzzy_matched: true, // 标记需要交互确认
    })
}

// ─── SdkInstallStrategy trait ────────────────────────────────────

pub trait SdkInstallStrategy: Send + Sync {
    /// SDK 特有：将 HTTP 响应解析为 VersionEntry 列表
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>>;

    /// SDK 特有：构建下载 URL（模板渲染或使用直链）
    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String>;
}

pub fn get_install_strategy(sdk: &Sdk) -> Box<dyn SdkInstallStrategy> {
    match sdk {
        Sdk::Built(BuiltinSdk::Java) => Box::new(JavaStrategy),
        Sdk::Built(BuiltinSdk::Node) => Box::new(NodeStrategy),
        Sdk::Built(BuiltinSdk::Python) => Box::new(PythonStrategy),
        Sdk::Built(BuiltinSdk::Maven) => Box::new(MavenStrategy),
        Sdk::Custom(_) => Box::new(ConfigBasedStrategy::default()),
    }
}

// ─── 通用 resolve 入口 ──────────────────────────────────────────

/// 通用版本解析入口：主备切换 + 缓存 + 模糊匹配
pub async fn resolve_sdk_version(
    client: &Client,
    strategy: &dyn SdkInstallStrategy,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    version_input: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<ResolvedVersion> {
    let body = fetch_version_data(client, source, cache_key, sdk_label, headers).await?;
    let entries = strategy.parse_version_data(&body)?;
    fuzzy_match_version(&entries, version_input)
}

// ─── Java Strategy ───────────────────────────────────────────────

struct JavaStrategy;

impl SdkInstallStrategy for JavaStrategy {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // Java 的 version_list_url 返回 available_releases（只有 major 版本号列表）
        // 需要二次查询 assets API 获取精确 semver — 此处只解析第一步
        let data: AdoptiumReleases = serde_json::from_str(body)
            .context("[Java version resolve] failed to parse Adoptium releases data")?;
        // Java 的第一步只验证 major 版本是否可用，精确 semver 需要额外逻辑
        // 这里返回 major 版本号作为 entry，后续由 resolve_java_version 处理两步逻辑
        let entries: Vec<VersionEntry> = data.available_releases.iter()
            .map(|v| VersionEntry {
                full_version: v.to_string(),
                feature_version: Some(v.to_string()),
                release_tag: None,
                download_url: None,
            })
            .collect();
        Ok(entries)
    }

    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String> {
        let mut r = TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Adoptium))
            .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Adoptium))
            .var(PLACEHOLDER_OS_EXT, detect_ext());
        if let Some(fv) = &resolved.feature_version {
            r = r.var(PLACEHOLDER_FEATURE_VERSION, fv);
        }
        r.render(template)
    }
}

/// Java 的两步解析逻辑（available_releases → assets API）
/// 因为 Java 需要两步查询，不能完全套用通用流程，保留独立实现
pub async fn resolve_java_version(client: &Client, version_input: &str) -> Result<ResolvedVersion> {
    let config = find_builtin_sdk_config(&BuiltinSdk::Java)
        .context("[Java version resolve] no builtin SDK config for Java")?;

    // Step 1: resolve available major versions from Adoptium
    let resp = client.get(config.version_url).send().await
        .context("[Java version resolve] failed to query Adoptium releases API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("[Java version resolve] Adoptium releases API returned HTTP {}, response: {}", status, truncate(&body, 200));
    }

    let data: AdoptiumReleases = resp.json().await
        .context("[Java version resolve] failed to parse Adoptium releases data")?;

    let input_num: i32 = version_input.parse()
        .context(format!("[Java version resolve] '{}' is not a valid Java major version, use an integer like 21/17/11", version_input))?;

    if !data.available_releases.contains(&input_num) {
        bail!("[Java version resolve] Java {} not available in Adoptium, available: {:?}", version_input, data.available_releases);
    }

    // Step 2: resolve exact semver via Adoptium assets API
    let assets_url = TemplateRenderer::new()
        .var(PLACEHOLDER_FEATURE_VERSION, version_input)
        .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Adoptium))
        .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Adoptium))
        .render(config.assets_url.context("[Java version resolve] assets_url not configured in builtin SDK config")?)?;

    let resp = client.get(&assets_url).send().await
        .context("[Java version resolve] failed to query Adoptium assets API for exact version")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("[Java version resolve] Adoptium assets API returned HTTP {}, response: {}", status, truncate(&body, 200));
    }

    let assets: Vec<AdoptiumAsset> = resp.json().await
        .context("[Java version resolve] failed to parse Adoptium assets data")?;

    if assets.is_empty() {
        bail!("[Java version resolve] no JDK release found for Java {} on {} {}", input_num, detect_os_with(OsStyle::Adoptium), detect_arch_with(ArchStyle::Adoptium));
    }

    let full_version = assets[0].version.semver.split('+').next().unwrap_or(&assets[0].version.semver).to_string();

    Ok(ResolvedVersion {
        full_version,
        feature_version: Some(version_input.to_string()),
        release_tag: None,
        download_url: None,
        fuzzy_matched: false,
    })
}

#[derive(Debug, Deserialize)]
struct AdoptiumReleases {
    available_releases: Vec<i32>,
    #[allow(dead_code)]
    most_recent_lts_version: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset { version: AdoptiumVersion }
#[derive(Debug, Deserialize)]
struct AdoptiumVersion { semver: String }

// ─── Node Strategy ───────────────────────────────────────────────

struct NodeStrategy;

impl SdkInstallStrategy for NodeStrategy {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        let versions: Vec<NodeVersion> = serde_json::from_str(body)
            .context("[Node version lookup] failed to parse nodejs.org version data")?;

        // 转换为 VersionEntry，保留 LTS 标记用于优先选择
        let entries: Vec<VersionEntry> = versions.iter()
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

    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String> {
        TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Short))
            .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Default))
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, format!("v{}", resolved.full_version))
            .render(template)
    }
}

#[derive(Debug, Deserialize)]
struct NodeVersion { version: String, #[allow(dead_code)] lts: NodeLts }

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NodeLts { Bool(bool), String(String) }

// ─── Python Strategy ─────────────────────────────────────────────

struct PythonStrategy;

impl SdkInstallStrategy for PythonStrategy {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // 先尝试 uv download-metadata.json 格式（弹性解析，单条失败不影响整体）
        if let Ok(entries) = parse_uv_metadata(body) {
            if !entries.is_empty() {
                return Ok(entries);
            }
        }
        // 回退解析 GitHub API 格式（备源）
        parse_github_releases(body)
    }

    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String> {
        // 如果已有直链（来自 uv metadata），直接使用
        if let Some(url) = &resolved.download_url {
            return Ok(url.clone());
        }
        // 否则使用模板渲染
        let mut r = TemplateRenderer::new()
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .var(PLACEHOLDER_PLATFORM, detect_platform_triple()?);
        if let Some(tag) = &resolved.release_tag {
            r = r.var(PLACEHOLDER_RELEASE_TAG, tag);
        }
        r.render(template)
    }
}

/// 解析 uv download-metadata.json 格式（主源）
/// 使用 serde_json::Value 弹性解析：单条字段不匹配时跳过而非整体失败
fn parse_uv_metadata(body: &str) -> Result<Vec<VersionEntry>> {
    // 先弹性解析为 HashMap<String, Value>，不依赖严格结构体
    let metadata: HashMap<String, serde_json::Value> = serde_json::from_str(body)
        .context("[Python version sniff] response is not valid JSON")?;

    let platform_triple = detect_platform_triple()?;
    let target = map_uv_target(&platform_triple)?;

    let mut entries: Vec<VersionEntry> = Vec::new();

    for (_key, value) in &metadata {
        // 只处理 cpython stable 标准版本
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "cpython" { continue; }

        // variant 必须为 null 或不存在
        if let Some(variant) = value.get("variant") {
            if !variant.is_null() { continue; }
        }

        // prerelease 必须为空字符串或不存在
        let prerelease = value.get("prerelease")
            .and_then(|v| v.as_str()).unwrap_or("");
        if !prerelease.is_empty() { continue; }

        // 匹配当前平台
        let os = value.get("os").and_then(|v| v.as_str()).unwrap_or("");
        let libc = value.get("libc").and_then(|v| v.as_str()).unwrap_or("");
        let arch_family = value.get("arch")
            .and_then(|v| v.get("family"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if os != target.os || arch_family != target.arch || libc != target.libc { continue; }

        // 提取版本信息（用默认值避免字段缺失导致失败）
        let major = value.get("major").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let minor = value.get("minor").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let patch = value.get("patch").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let build = value.get("build").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if major == 0 || url.is_empty() { continue; }

        let full = format!("{}.{}.{}", major, minor, patch);
        entries.push(VersionEntry {
            full_version: full,
            feature_version: Some(major.to_string()),
            release_tag: Some(build),
            download_url: Some(url),
        });
    }

    // 按版本号从高到低排列（模糊匹配取第一个 = 最新）
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
        "x86_64-pc-windows-msvc"   => Ok(UvTarget { os: "windows",  arch: "x86_64",  libc: "none" }),
        "aarch64-pc-windows-msvc"  => Ok(UvTarget { os: "windows",  arch: "aarch64",  libc: "none" }),
        "i686-pc-windows-msvc"     => Ok(UvTarget { os: "windows",  arch: "i686",     libc: "none" }),
        "x86_64-apple-darwin"      => Ok(UvTarget { os: "darwin",   arch: "x86_64",   libc: "none" }),
        "aarch64-apple-darwin"     => Ok(UvTarget { os: "darwin",   arch: "aarch64",  libc: "none" }),
        "x86_64-unknown-linux-gnu" => Ok(UvTarget { os: "linux",    arch: "x86_64",   libc: "gnu" }),
        "aarch64-unknown-linux-gnu"=> Ok(UvTarget { os: "linux",    arch: "aarch64",  libc: "gnu" }),
        "x86_64-unknown-linux-musl"=> Ok(UvTarget { os: "linux",    arch: "x86_64",   libc: "musl" }),
        "aarch64-unknown-linux-musl"=> Ok(UvTarget { os: "linux",  arch: "aarch64",  libc: "musl" }),
        _ => bail!("[Python version sniff] unsupported platform '{}' for uv metadata", platform_triple),
    }
}

/// 解析 GitHub Releases API 格式（备源）
fn parse_github_releases(body: &str) -> Result<Vec<VersionEntry>> {
    let releases: Vec<GitHubRelease> = serde_json::from_str(body)
        .context("[Python version sniff] failed to parse GitHub releases data")?;

    let platform = detect_platform_triple()?;
    let suffix = format!("{}-install_only.tar.gz", platform);

    let mut entries: Vec<VersionEntry> = Vec::new();
    for release in &releases {
        for asset in &release.assets {
            if !asset.name.ends_with(&suffix) { continue; }
            let version = extract_python_version(&asset.name);
            if let Some(v) = version {
                let fv = v.split('.').next().map(|s| s.to_string());
                entries.push(VersionEntry {
                    full_version: v,
                    feature_version: fv,
                    release_tag: Some(release.tag_name.clone()),
                    download_url: None, // GitHub API 不提供直链，需模板渲染
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
struct GitHubRelease { tag_name: String, assets: Vec<GitHubAsset> }
#[derive(Debug, Deserialize)]
struct GitHubAsset { name: String, #[allow(dead_code)] size: u64 }

// ─── Maven Strategy ──────────────────────────────────────────────

struct MavenStrategy;

impl SdkInstallStrategy for MavenStrategy {
    fn parse_version_data(&self, _body: &str) -> Result<Vec<VersionEntry>> {
        // Maven 无远程版本发现，parse 不使用
        Ok(Vec::new())
    }

    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String> {
        TemplateRenderer::new()
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .render(template)
    }
}

// ─── ConfigBased Strategy (custom SDK) ───────────────────────────

#[derive(Default)]
pub struct ConfigBasedStrategy { os_style: OsStyle, arch_style: ArchStyle }

impl SdkInstallStrategy for ConfigBasedStrategy {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // 自定义 SDK 尝试自动解析常见 JSON 版本格式：
        // 1. 扁平字符串数组：["3.12.8", "3.12.7", "3.11.12"]
        // 2. 对象数组含 version 字段：[{"version": "3.12.8"}, ...]
        if let Ok(entries) = parse_flat_version_array(body) {
            return Ok(entries);
        }
        if let Ok(entries) = parse_version_object_array(body) {
            return Ok(entries);
        }
        bail!("[Custom SDK version validate] failed to auto-parse version data from configured version_url. \
               Supported formats: flat string array or array of objects with 'version' field");
    }

    fn build_download_url(&self, template: &str, resolved: &ResolvedVersion) -> Result<String> {
        TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(self.os_style))
            .var(PLACEHOLDER_ARCH, detect_arch_with(self.arch_style))
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .render(template)
    }
}

/// 解析扁平版本字符串数组
fn parse_flat_version_array(body: &str) -> Result<Vec<VersionEntry>> {
    let versions: Vec<String> = serde_json::from_str(body)
        .context("not a flat string array")?;
    let mut entries: Vec<VersionEntry> = versions.iter()
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
    let items: Vec<GenericVersionObject> = serde_json::from_str(body)
        .context("not an object array with version field")?;
    let mut entries: Vec<VersionEntry> = items.iter()
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

// ─── Helpers ─────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { s[..max].to_string() }
}
