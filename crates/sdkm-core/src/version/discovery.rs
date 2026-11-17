// 各 SDK 版本发现 + 解析编排(install/list 共用)
//
// 注意:下载 URL 构建(`build_download_url`)已移至 `install/download_url.rs`,
// 本模块只负责"版本发现"(把 HTTP 响应解析为 VersionEntry 列表)+ 解析编排。

use crate::config::NetworkConfig;
use crate::report::EventSink;
use anyhow::{Context, Result, bail};
use regex_lite::Regex;
use reqwest::Client;
use sdkm_util::builtin::find_seed;
use sdkm_util::config_helper::{ArchStyle, OsStyle, detect_arch_with, detect_os_with, detect_platform_triple};
use sdkm_util::sdk::{BuiltinSdk, Sdk};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::{Arc, OnceLock};

use super::cache::{VersionSource, fetch_version_data};
use super::fuzzy::{JavaVendor, format_java_version, fuzzy_match_version_core, parse_java_version_input};
use super::github_releases::{GitHubRelease, github_api_headers, parse_releases};
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

/// Java/Tomcat 占位解析器：两家走多请求专属路径（install/list 提前分流），不应落到通用解析
struct MultiRequestDiscovery;

impl VersionDiscovery for MultiRequestDiscovery {
    fn parse_version_data(&self, _body: &str) -> Result<Vec<VersionEntry>> {
        bail!("this SDK uses a dedicated multi-request discovery path; the generic parser is unreachable")
    }
}

/// 版本发现分发：专属路径 SDK 按枚举固定，其余走 ConfigBased 通用解析
///
/// ConfigBased 依次识别三种 JSON 形状：扁平字符串数组 / {version} 对象数组 /
/// GitHub Releases 数组（标准 B，`asset_prefix` 作资产前缀门：内置种子取主可执行
/// 文件名 claude-code→"claude"，自定义 SDK 取名字本身）。
/// GH 形状靠 JSON 结构识别而非 URL——本地镜像/代理/测试 server 同样生效。
pub fn get_version_discovery(sdk: &Sdk, _version_url: &str) -> Box<dyn VersionDiscovery> {
    match sdk {
        Sdk::Built(BuiltinSdk::Java) | Sdk::Built(BuiltinSdk::Tomcat) => Box::new(MultiRequestDiscovery),
        Sdk::Built(BuiltinSdk::Node) => Box::new(NodeDiscovery),
        Sdk::Built(BuiltinSdk::Python) => Box::new(PythonDiscovery),
        Sdk::Built(BuiltinSdk::Maven) => Box::new(MavenDiscovery),
        Sdk::Built(BuiltinSdk::Go) => Box::new(GoDiscovery),
        _ => {
            let sdk_name = sdk.to_string();
            let prefix = find_seed(&sdk_name)
                .map(|s| s.asset_prefix.unwrap_or(s.primary_executables[0]))
                .unwrap_or(&sdk_name);
            Box::new(ConfigBasedDiscovery {
                asset_prefix: prefix.to_string(),
            })
        }
    }
}

// ─── 通用 resolve 入口 ──────────────────────────────────────────

/// 通用版本解析入口:主备切换 + 缓存 + 模糊匹配
#[allow(clippy::too_many_arguments)] // 解析编排参数天然多（同 fetch_version_data），打包反而模糊
pub async fn resolve_sdk_version(
    client: &Client,
    discovery: &dyn VersionDiscovery,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    version_input: &str,
    headers: Option<HashMap<String, String>>,
    cache_ttl_secs: u64,
    network: &NetworkConfig,
    sink: &dyn EventSink,
) -> Result<ResolvedVersion> {
    let body = fetch_version_data(client, source, cache_key, sdk_label, headers, cache_ttl_secs, network, sink).await?;
    let entries = discovery.parse_version_data(&body)?;
    fuzzy_match_version(&entries, version_input)
}

// ─── Java 多供应商版本发现 ──────────────────────────────────────
//
// 供应商标识走 sdkman 式后缀（store 目录名 `21.0.13-zulu`），输入支持两种等价写法：
// `21-zulu` / `zulu@21`（解析在 fuzzy.rs 的 parse_java_version_input）。
// 每家的抓取都经 fetch_version_data（缓存/重试/TTL），下载直链逐条携带。

/// 单条供应商版本（数字版本号 + 平台下载直链）
struct JavaRelease {
    numeric: String,
    download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdoptiumInfo {
    #[allow(dead_code)]
    available_releases: Vec<u32>,
    available_lts_releases: Vec<u32>,
    most_recent_feature_release: u32,
}

#[derive(Debug, Deserialize)]
struct TemurinRelease {
    #[serde(default)]
    version_data: Option<TemurinSemver>,
    binaries: Vec<TemurinBinary>,
}
#[derive(Debug, Deserialize)]
struct TemurinSemver {
    semver: String,
}
#[derive(Debug, Deserialize)]
struct TemurinBinary {
    os: String,
    architecture: String,
    package: TemurinPackage,
}
#[derive(Debug, Deserialize)]
struct TemurinPackage {
    link: String,
}

#[derive(Debug, Deserialize)]
struct ZuluPackage {
    name: String,
    download_url: String,
    #[serde(default)]
    java_version: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct LibericaRelease {
    version: String,
    #[serde(rename = "downloadUrl")]
    download_url: String,
}

/// 供应商 API 的 OS 命名（windows/linux/macos，四家一致）
fn vendor_os_name() -> &'static str {
    match env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    }
}

/// 供应商 API 的 arch 命名（x64/aarch64，四家一致）
fn vendor_arch_name() -> Result<&'static str> {
    match env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "aarch64" => Ok("aarch64"),
        other => bail!("[Java version resolve] unsupported arch '{}' for vendor APIs", other),
    }
}

/// 压缩包扩展名（win/mac zip，linux tar.gz）
fn vendor_ext() -> &'static str {
    if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        "zip"
    } else {
        "tar.gz"
    }
}

/// 数字版本降序排序（点段逐段比较，非数字段忽略）
fn sort_java_releases_desc(releases: &mut [JavaRelease]) {
    releases.sort_by(|a, b| {
        let va: Vec<u32> = a.numeric.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.numeric.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
}

/// Temurin：Adoptium feature_releases API（返回该 major 的全部 patch + 平台直链）
async fn fetch_temurin(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    major: u32,
) -> Result<Vec<JavaRelease>> {
    let os = detect_os_with(OsStyle::Adoptium);
    let arch = detect_arch_with(ArchStyle::Adoptium);
    let url = format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?architecture={arch}&image_type=jdk&os={os}&page=0&page_size=30&project=jdk&sort_order=DESC&vendor=eclipse"
    );
    let body = fetch_version_data(
        client,
        &VersionSource {
            primary_url: url,
            secondary_url: None,
        },
        &format!("java-temurin-{major}"),
        "java",
        None,
        ttl,
        network,
        sink,
    )
    .await?;
    parse_temurin_releases(&body)
}

/// 解析 Temurin feature_releases：semver 去掉 `+build`，只保留当前平台有包的版本
fn parse_temurin_releases(body: &str) -> Result<Vec<JavaRelease>> {
    let releases: Vec<TemurinRelease> =
        serde_json::from_str(body).context("[Java version lookup] failed to parse Temurin feature releases data")?;
    let os = detect_os_with(OsStyle::Adoptium);
    let arch = detect_arch_with(ArchStyle::Adoptium);

    let mut out: Vec<JavaRelease> = Vec::new();
    for r in releases {
        // 查询参数已按平台过滤，这里兜底再选一次；无当前平台包的版本不进列表
        let Some(link) = r
            .binaries
            .iter()
            .find(|b| b.os == os && b.architecture == arch)
            .map(|b| b.package.link.clone())
        else {
            continue;
        };
        let semver = r.version_data.as_ref().map(|v| v.semver.clone()).unwrap_or_default();
        if semver.is_empty() {
            continue;
        }
        let numeric = semver.split('+').next().unwrap_or(&semver).to_string();
        out.push(JavaRelease {
            numeric,
            download_url: Some(link),
        });
    }
    sort_java_releases_desc(&mut out);
    Ok(out)
}

/// Adoptium available_releases（major 宇宙默认源；config 的 `sdk.java.version_url` 优先）
const ADOPTIUM_INFO_URL: &str = "https://api.adoptium.net/v3/info/available_releases";

/// 信息源主备都失败时的 LTS 兜底（新增 LTS 时更新；正常路径不会用到）
const JAVA_DEFAULT_LTS_MAJORS: &[u32] = &[8, 11, 17, 21, 25];

/// 信息源主备：config `version_url` 优先，为空回退内置 Adoptium；备源去空/去重
fn adoptium_major_sources(info_url: &str, info_fallback_url: Option<&str>) -> VersionSource {
    let primary_url = if info_url.trim().is_empty() {
        ADOPTIUM_INFO_URL.to_string()
    } else {
        info_url.trim().to_string()
    };
    let secondary_url = info_fallback_url
        .map(str::trim)
        .filter(|u| !u.is_empty() && *u != primary_url)
        .map(str::to_string);
    VersionSource {
        primary_url,
        secondary_url,
    }
}

/// Adoptium 可用 major 信息（主源 = config `version_url`，备源 = config `version_fallback_url`）
async fn fetch_adoptium_majors(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    info_url: &str,
    info_fallback_url: Option<&str>,
) -> Result<Vec<u32>> {
    let body = fetch_version_data(
        client,
        &adoptium_major_sources(info_url, info_fallback_url),
        "java",
        "java",
        None,
        ttl,
        network,
        sink,
    )
    .await?;
    let info: AdoptiumInfo =
        serde_json::from_str(&body).context("[Java version lookup] failed to parse Adoptium releases data")?;
    // LTS 系列 + 最新 feature major；去重升序
    let mut majors = info.available_lts_releases;
    if !majors.contains(&info.most_recent_feature_release) {
        majors.push(info.most_recent_feature_release);
    }
    majors.sort_unstable();
    Ok(majors)
}

/// 列表用 major 集合：主源/备源都失败时告警并退回内置 LTS 列表，保证 `list java -r` 仍可用
pub async fn fetch_java_list_majors(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    info_url: &str,
    info_fallback_url: Option<&str>,
) -> Vec<u32> {
    match fetch_adoptium_majors(client, network, ttl, sink, info_url, info_fallback_url).await {
        Ok(majors) => majors,
        Err(e) => {
            sink.warning(format!(
                "[java] failed to fetch available releases, using default LTS majors: {}",
                truncate(&e.to_string(), 160)
            ));
            JAVA_DEFAULT_LTS_MAJORS.to_vec()
        }
    }
}

/// Zulu：Azul packages API。怪癖：`latest=true` 只含每个 major 最新、`latest=false`
/// 含历史但排除最新，两次请求合并才是全量（无需 java_version 过滤，客户端按 major 筛）
async fn fetch_zulu(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    major: Option<u32>,
) -> Result<Vec<JavaRelease>> {
    let os = vendor_os_name();
    let arch = vendor_arch_name()?;
    let ext = vendor_ext();

    let mut all: Vec<ZuluPackage> = Vec::new();
    for latest in [true, false] {
        let url = format!(
            "https://api.azul.com/metadata/v1/zulu/packages/?os={os}&arch={arch}&archive_type={ext}&java_package_type=jdk&javafx_bundled=false&crac_supported=false&release_status=ga&availability_types=CA&latest={latest}&page=1&page_size=300"
        );
        let key = if latest {
            "java-zulu-latest"
        } else {
            "java-zulu-history"
        };
        let body = fetch_version_data(
            client,
            &VersionSource {
                primary_url: url,
                secondary_url: None,
            },
            key,
            "java",
            None,
            ttl,
            network,
            sink,
        )
        .await?;
        let packages: Vec<ZuluPackage> =
            serde_json::from_str(&body).context("[Java version lookup] failed to parse Zulu packages data")?;
        all.extend(packages);
    }

    // 过滤 fx/crac 变体与 musl/glibc 错配；同 numeric 保留 build 最高的包
    Ok(zulu_releases(all, major, cfg!(target_env = "musl")))
}

/// Zulu 包过滤 + 去重（纯函数，供单测）：同 numeric 保留 java_version 最大的（最新 build）
fn zulu_releases(all: Vec<ZuluPackage>, major: Option<u32>, is_musl: bool) -> Vec<JavaRelease> {
    let mut best: HashMap<String, (Vec<u32>, String)> = HashMap::new();
    for p in all {
        if !p.name.contains("-ca-jdk") || p.name.contains("-fx-") || p.name.contains("-crac-") {
            continue;
        }
        if p.name.contains("linux_musl") != is_musl {
            continue;
        }
        if p.java_version.is_empty() {
            continue;
        }
        if let Some(m) = major
            && p.java_version[0] != m
        {
            continue;
        }
        let numeric = p
            .java_version
            .iter()
            .take(3)
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(".");
        let better = best
            .get(&numeric)
            .is_none_or(|(old, _)| p.java_version.as_slice() > old.as_slice());
        if better {
            best.insert(numeric, (p.java_version, p.download_url));
        }
    }
    let mut out: Vec<JavaRelease> = best
        .into_iter()
        .map(|(numeric, (_, url))| JavaRelease {
            numeric,
            download_url: Some(url),
        })
        .collect();
    sort_java_releases_desc(&mut out);
    out
}

/// Corretto：GitHub corretto-{major} 仓库的 tag 列表（无 assets）+ corretto.aws 模板拼直链
async fn fetch_corretto(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    major: u32,
) -> Result<Vec<JavaRelease>> {
    // Windows aarch64 无官方包，提前给出可行动的错误
    if cfg!(target_os = "windows") && cfg!(target_arch = "aarch64") {
        bail!(
            "[Java version resolve] Corretto does not publish Windows aarch64 builds; try another vendor (tem/zulu/librca)"
        );
    }
    let os = vendor_os_name();
    let arch = vendor_arch_name()?;
    let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };

    let mut out: Vec<JavaRelease> = Vec::new();
    // 100 条/页足够覆盖单个 major 的全部 release；满页再翻一页兜底
    for page in 1..=2u32 {
        let url = format!("https://api.github.com/repos/corretto/corretto-{major}/releases?per_page=100&page={page}");
        let body = fetch_version_data(
            client,
            &VersionSource {
                primary_url: url,
                secondary_url: None,
            },
            &format!("java-corretto-{major}-p{page}"),
            "java",
            Some(github_api_headers()),
            ttl,
            network,
            sink,
        )
        .await?;
        let releases = parse_corretto_tags(&body, os, arch, ext)?;
        let full_page = releases.len() >= 100;
        out.extend(releases);
        if !full_page {
            break;
        }
    }
    sort_java_releases_desc(&mut out);
    Ok(out)
}

/// 解析 Corretto tag 列表：只收纯数字点分 tag，按平台拼 corretto.aws 直链
fn parse_corretto_tags(body: &str, os: &str, arch: &str, ext: &str) -> Result<Vec<JavaRelease>> {
    let releases: Vec<GitHubRelease> =
        serde_json::from_str(body).context("[Java version lookup] failed to parse Corretto releases data")?;
    let mut out: Vec<JavaRelease> = Vec::new();
    for r in releases {
        if r.prerelease || r.draft {
            continue;
        }
        let tag = r.tag_name;
        // 仅纯数字点分 tag（排除源码 tag 等异常形态）
        if tag.is_empty() || !tag.chars().all(|c| c.is_ascii_digit() || c == '.') {
            continue;
        }
        let url = if os == "windows" {
            format!("https://corretto.aws/downloads/resources/{tag}/amazon-corretto-{tag}-windows-{arch}-jdk.{ext}")
        } else {
            // Corretto 归档目录用 macosx（不是 macos）
            let url_os = if os == "macos" { "macosx" } else { os };
            format!("https://corretto.aws/downloads/resources/{tag}/amazon-corretto-{tag}-{url_os}-{arch}.{ext}")
        };
        out.push(JavaRelease {
            numeric: tag,
            download_url: Some(url),
        });
    }
    Ok(out)
}

/// Liberica：BellSoft API（必须带 version-feature，否则返回 16MB 全量）
async fn fetch_liberica(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    major: u32,
) -> Result<Vec<JavaRelease>> {
    let os = vendor_os_name();
    let arch = if cfg!(target_arch = "aarch64") { "arm" } else { "x86" };
    let ext = vendor_ext();
    let url = format!(
        "https://api.bell-sw.com/v1/liberica/releases?version-feature={major}&os={os}&arch={arch}&package-type={ext}&bundle-type=jdk&bitness=64&output=json"
    );
    let body = fetch_version_data(
        client,
        &VersionSource {
            primary_url: url,
            secondary_url: None,
        },
        &format!("java-liberica-{major}"),
        "java",
        None,
        ttl,
        network,
        sink,
    )
    .await?;
    parse_liberica_releases(&body)
}

/// 解析 Liberica：`21.0.13+11` → `21.0.13`，同版本多 build 保留最高
fn parse_liberica_releases(body: &str) -> Result<Vec<JavaRelease>> {
    let releases: Vec<LibericaRelease> =
        serde_json::from_str(body).context("[Java version lookup] failed to parse Liberica releases data")?;
    let mut best: HashMap<String, (Vec<u32>, String)> = HashMap::new();
    for r in releases {
        let numeric = r.version.split('+').next().unwrap_or(&r.version).to_string();
        if numeric.is_empty() {
            continue;
        }
        let full: Vec<u32> = r.version.replace('+', ".").split('.').filter_map(|s| s.parse().ok()).collect();
        let better = best.get(&numeric).is_none_or(|(old, _)| full > *old);
        if better {
            best.insert(numeric, (full, r.download_url));
        }
    }
    let mut out: Vec<JavaRelease> = best
        .into_iter()
        .map(|(numeric, (_, url))| JavaRelease {
            numeric,
            download_url: Some(url),
        })
        .collect();
    sort_java_releases_desc(&mut out);
    Ok(out)
}

/// 按供应商拉取指定 major 的版本列表
async fn fetch_java_vendor_versions(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    vendor: JavaVendor,
    major: u32,
) -> Result<Vec<JavaRelease>> {
    match vendor {
        JavaVendor::Temurin => fetch_temurin(client, network, ttl, sink, major).await,
        JavaVendor::Zulu => fetch_zulu(client, network, ttl, sink, Some(major)).await,
        JavaVendor::Corretto => fetch_corretto(client, network, ttl, sink, major).await,
        JavaVendor::Liberica => fetch_liberica(client, network, ttl, sink, major).await,
    }
}

/// Java 安装解析：输入 `21` / `21.0.13` / `21-zulu` / `zulu@21` → 规范版本 + 直链
pub async fn resolve_java_version(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    version_input: &str,
) -> Result<ResolvedVersion> {
    let (numeric, vendor) = parse_java_version_input(version_input)?;
    let Some(major) = numeric.split('.').next().and_then(|s| s.parse::<u32>().ok()) else {
        bail!(
            "'{}' is not a valid Java version, use a major like 21, exact like 21.0.13, or vendor form like 21-zulu / zulu@21",
            version_input
        );
    };

    let releases = fetch_java_vendor_versions(client, network, ttl, sink, vendor, major).await?;
    if releases.is_empty() {
        bail!(
            "[Java version resolve] no {} {} builds for {} {} — the vendor doesn't ship this version for this platform; try another version or vendor",
            vendor.display_name(),
            major,
            vendor_os_name(),
            vendor_arch_name()?
        );
    }

    // 裸 major = 该系列最新（不触发确认，保持既有 UX）；部分/精确版本走模糊匹配
    let (matched_numeric, download_url, fuzzy_matched) = if !numeric.contains('.') {
        let r = &releases[0];
        // Temurin 裸 major 走 config 下载模板（Adoptium binary/latest API，可 `config set` 换镜像）；
        // 其他供应商与精确版本号用 API 返回的平台直链
        let url = if matches!(vendor, JavaVendor::Temurin) {
            None
        } else {
            r.download_url.clone()
        };
        (r.numeric.clone(), url, false)
    } else {
        let nums: Vec<String> = releases.iter().map(|r| r.numeric.clone()).collect();
        let m = fuzzy_match_version_core(&nums, &numeric)?;
        let r = releases.iter().find(|r| r.numeric == m.full_version).context(format!(
            "[Java version resolve] matched version '{}' not found in vendor list (internal error)",
            m.full_version
        ))?;
        (r.numeric.clone(), r.download_url.clone(), m.fuzzy_matched)
    };

    Ok(ResolvedVersion {
        full_version: format_java_version(&matched_numeric, vendor),
        feature_version: Some(major.to_string()),
        release_tag: None,
        download_url,
        fuzzy_matched,
    })
}

/// 远程列表：四家供应商 × majors 并行拉取，合并为带供应商后缀的统一列表
///
/// 单家失败仅告警不阻断（其他家照常展示）；全部失败才报错。
pub async fn list_java_versions(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: Arc<dyn EventSink>,
    majors: &[u32],
) -> Result<Vec<VersionEntry>> {
    let mut handles = Vec::new();
    let majors_vec: Vec<u32> = majors.to_vec();
    // Zulu 无需按 major 查询（全量拉一次再筛），单独一个任务
    {
        let client = client.clone();
        let network = network.clone();
        let sink = sink.clone();
        handles.push(tokio::spawn(async move {
            fetch_zulu(&client, &network, ttl, sink.as_ref(), None)
                .await
                .map(|rs| rs.into_iter().map(|r| (r, JavaVendor::Zulu)).collect::<Vec<_>>())
        }));
    }
    for m in majors_vec {
        for v in [JavaVendor::Temurin, JavaVendor::Corretto, JavaVendor::Liberica] {
            let client = client.clone();
            let network = network.clone();
            let sink = sink.clone();
            handles.push(tokio::spawn(async move {
                fetch_java_vendor_versions(&client, &network, ttl, sink.as_ref(), v, m)
                    .await
                    .map(|rs| rs.into_iter().map(|r| (r, v)).collect::<Vec<_>>())
            }));
        }
    }

    let mut releases: Vec<(JavaRelease, JavaVendor)> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for h in handles {
        match h.await {
            Ok(Ok(rs)) => releases.extend(rs),
            Ok(Err(e)) => errors.push(e.to_string()),
            Err(e) => errors.push(format!("task join failed: {e}")),
        }
    }
    if releases.is_empty() {
        bail!("[Java version lookup] all vendor sources failed: {}", errors.join("; "));
    }
    for e in &errors {
        sink.warning(format!("[java] one vendor source failed, skipping: {}", truncate(e, 160)));
    }

    // Zulu 全量结果按 majors 过滤
    releases.retain(|(r, v)| {
        *v != JavaVendor::Zulu
            || r.numeric
                .split('.')
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .is_some_and(|m| majors.contains(&m))
    });

    // 组装规范后缀条目：版本降序，同版本按 Temurin→Zulu→Corretto→Liberica
    let vendor_rank = |v: JavaVendor| JavaVendor::ALL.iter().position(|x| *x == v).unwrap_or(99);
    releases.sort_by(|(ra, va), (rb, vb)| {
        let na: Vec<u32> = ra.numeric.split('.').filter_map(|s| s.parse().ok()).collect();
        let nb: Vec<u32> = rb.numeric.split('.').filter_map(|s| s.parse().ok()).collect();
        nb.cmp(&na).then(vendor_rank(*va).cmp(&vendor_rank(*vb)))
    });

    let mut entries: Vec<VersionEntry> = Vec::new();
    let mut seen = HashSet::new();
    for (r, v) in releases {
        let full = format_java_version(&r.numeric, v);
        if !seen.insert(full.clone()) {
            continue;
        }
        let feature = r.numeric.split('.').next().map(|s| s.to_string());
        entries.push(VersionEntry {
            full_version: full,
            feature_version: feature,
            release_tag: None,
            download_url: r.download_url,
        });
    }
    Ok(entries)
}

// ─── Tomcat 版本发现 ────────────────────────────────────────────
//
// Apache 归档是两级 HTML 目录索引（root 列分支 → 分支列 v 版本），无 JSON 接口；
// 通用引擎是"单 body 解析"，故这里做多请求编排：root + 各分支都走 fetch_version_data
// （缓存 key `tomcat` / `tomcat-{n}`）。下载走 config 模板（download_url=None），镜像可覆盖。

/// 支持的 Tomcat 主版本分支
const TOMCAT_SUPPORTED_FEATURES: &[u32] = &[9, 10, 11];

/// 归档根索引里的分支目录链接 `tomcat-{N}/`
fn tomcat_branch_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"tomcat-(\d+)/"#).expect("valid regex"))
}

/// 分支索引里的版本目录链接 `vX.Y.Z/`（严格三段 + 结尾斜杠，里程碑 `v11.0.0-M1/` 天然不匹配）
fn tomcat_version_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"v(\d+\.\d+\.\d+)/"#).expect("valid regex"))
}

/// 解析归档根索引：`tomcat-{N}/` 分支目录（只保留支持的分支）
fn parse_tomcat_root_listing(body: &str) -> Vec<u32> {
    let mut feats: Vec<u32> = tomcat_branch_re()
        .captures_iter(body)
        .filter_map(|c| c[1].parse().ok())
        .filter(|f| TOMCAT_SUPPORTED_FEATURES.contains(f))
        .collect();
    feats.sort_unstable();
    feats.dedup();
    feats
}

/// 解析分支索引：`vX.Y.Z/` 目录（里程碑与非常规命名不匹配即被排除，`-M` 再兜底一道）
///
/// `feature_version` 取版本号首段：Apache 归档分支目录就是主版本号（9.0.x→`tomcat-9`、
/// 10.1.x→`tomcat-10`、11.0.x→`tomcat-11`），无需从 URL 或调用方传入
fn parse_tomcat_branch_listing(body: &str) -> Result<Vec<VersionEntry>> {
    let mut entries: Vec<VersionEntry> = tomcat_version_re()
        .captures_iter(body)
        .filter(|c| !c[0].contains("-M"))
        .map(|c| VersionEntry {
            full_version: c[1].to_string(),
            feature_version: c[1].split('.').next().map(|s| s.to_string()),
            release_tag: None,
            download_url: None,
        })
        .collect();
    // 数字降序（与通用列表语义一致）
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
    Ok(entries)
}

/// 去掉 URL 尾部斜杠（统一 root/分支 URL 拼接）
fn trim_slash(u: &str) -> &str {
    u.trim_end_matches('/')
}

/// 拉取 Tomcat 全量版本列表（root 索引 → 逐分支索引）
pub async fn fetch_tomcat_entries(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    version_root: &str,
    version_fallback: Option<&str>,
) -> Result<Vec<VersionEntry>> {
    let root_body = fetch_version_data(
        client,
        &VersionSource {
            primary_url: format!("{}/", trim_slash(version_root)),
            secondary_url: version_fallback.map(|u| format!("{}/", trim_slash(u))),
        },
        "tomcat",
        "tomcat",
        None,
        ttl,
        network,
        sink,
    )
    .await?;

    let features = parse_tomcat_root_listing(&root_body);
    let mut entries: Vec<VersionEntry> = Vec::new();
    if features.is_empty() {
        // 容错：version_url 直接指向单分支索引（如 .../tomcat-11/），root body 就是版本列表
        entries = parse_tomcat_branch_listing(&root_body)?;
    } else {
        for f in features {
            let body = fetch_version_data(
                client,
                &VersionSource {
                    primary_url: format!("{}/tomcat-{f}/", trim_slash(version_root)),
                    secondary_url: version_fallback.map(|u| format!("{}/tomcat-{f}/", trim_slash(u))),
                },
                &format!("tomcat-{f}"),
                "tomcat",
                None,
                ttl,
                network,
                sink,
            )
            .await?;
            entries.extend(parse_tomcat_branch_listing(&body)?);
        }
    }

    // 跨分支去重（正常不重叠，兜底防御）+ 降序
    let mut seen = HashSet::new();
    entries.retain(|e| seen.insert(e.full_version.clone()));
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
    Ok(entries)
}

/// Tomcat 安装解析：拉全量列表后模糊匹配；feature_version 用条目值（major），
/// 避免用户输入 `9.0` 这类部分版本把模板拼成 `tomcat-9.0`
pub async fn resolve_tomcat_version(
    client: &Client,
    network: &NetworkConfig,
    ttl: u64,
    sink: &dyn EventSink,
    version_root: &str,
    version_fallback: Option<&str>,
    version_input: &str,
) -> Result<ResolvedVersion> {
    let entries = fetch_tomcat_entries(client, network, ttl, sink, version_root, version_fallback).await?;
    if entries.is_empty() {
        bail!(
            "[Tomcat version lookup] no tomcat versions found at {version_root} — the mirror must expose an Apache-style HTML directory listing"
        );
    }
    let versions: Vec<String> = entries.iter().map(|e| e.full_version.clone()).collect();
    let matched = fuzzy_match_version_core(&versions, version_input)?;
    let entry = entries
        .iter()
        .find(|e| e.full_version == matched.full_version)
        .context("[Tomcat version lookup] matched version not found in entries (internal error)")?;
    Ok(ResolvedVersion {
        full_version: matched.full_version,
        feature_version: entry.feature_version.clone(),
        release_tag: None,
        download_url: None,
        fuzzy_matched: matched.fuzzy_matched,
    })
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

/// 解析 GitHub Releases API 格式(Python 备源)
///
/// 复用标准 B 引擎的 JSON 结构;资产筛选保留 Python 专属语义
/// (仅匹配 `{platform}-install_only.tar.gz` 资产,版本号从资产名提取)
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

// GH Releases JSON 结构体复用 github_releases 模块的 GitHubRelease/GitHubAsset

// ─── Maven 版本发现 ─────────────────────────────────────────────

struct MavenDiscovery;

impl VersionDiscovery for MavenDiscovery {
    fn parse_version_data(&self, _body: &str) -> Result<Vec<VersionEntry>> {
        // Maven 无远程版本发现,parse 不使用
        Ok(Vec::new())
    }
}

// ─── Go 版本发现 ────────────────────────────────────────────────

struct GoDiscovery;

impl VersionDiscovery for GoDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        let versions: Vec<GoVersion> =
            serde_json::from_str(body).context("[Go version lookup] failed to parse go.dev version data")?;

        // 平台过滤与下载 URL 的 {os}/{arch} 渲染同源:只保留 files 含当前平台
        // archive 安装包的版本(go1 条目仅含 bootstrap 源码,天然被滤掉,列表=可下载)
        let cur_os = detect_os_with(OsStyle::Default);
        let cur_arch = detect_arch_with(ArchStyle::Go);
        // 去掉 go 前缀；feature_version 取 major.minor（如 1.26）
        let entries: Vec<VersionEntry> = versions
            .iter()
            .filter(|v| v.stable)
            .filter(|v| {
                v.files
                    .iter()
                    .any(|f| f.kind == "archive" && f.os == cur_os && f.arch == cur_arch)
            })
            .map(|v| {
                let ver = v.version.trim_start_matches("go").to_string();
                let fv = ver.split('.').take(2).collect::<Vec<_>>().join(".");
                VersionEntry {
                    full_version: ver,
                    feature_version: Some(fv),
                    release_tag: None,
                    download_url: None,
                }
            })
            .collect();
        Ok(entries)
    }
}

#[derive(Debug, Deserialize)]
struct GoVersion {
    version: String,
    stable: bool,
    files: Vec<GoFile>,
}

#[derive(Debug, Deserialize)]
struct GoFile {
    #[allow(dead_code)]
    filename: String,
    os: String,
    arch: String,
    kind: String,
}

// ─── ConfigBased 版本发现(自定义 SDK)──────────────────────────

/// 标准 A/B 通用解析：自动识别三种 JSON 版本格式
struct ConfigBasedDiscovery {
    /// GH Releases 形状识别时的资产前缀门（主可执行文件名）
    asset_prefix: String,
}

impl VersionDiscovery for ConfigBasedDiscovery {
    fn parse_version_data(&self, body: &str) -> Result<Vec<VersionEntry>> {
        // 依次尝试三种 JSON 形状：
        // 1. 扁平字符串数组:["3.12.8", ...]
        // 2. 对象数组含 version 字段:[{"version": "3.12.8"}, ...]
        // 3. GitHub Releases 数组（标准 B：tag 剥前缀 + 平台资产直链，asset_prefix 做前缀门）
        if let Ok(entries) = parse_flat_version_array(body) {
            return Ok(entries);
        }
        if let Ok(entries) = parse_version_object_array(body) {
            return Ok(entries);
        }
        if let Ok(entries) = parse_releases(body, &self.asset_prefix) {
            return Ok(entries);
        }
        bail!(
            "[Custom SDK version validate] failed to auto-parse version data from configured version_url.              Supported formats: flat string array, objects with 'version' field, or GitHub Releases array"
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tomcat 目录索引解析 ─────────────────────────────────────

    #[test]
    fn tomcat_root_listing_keeps_supported_branches() {
        let html = r#"
            <a href="tomcat-9/">v9</a>
            <a href="tomcat-10/">v10</a>
            <a href="tomcat-11/">v11</a>
            <a href="tomcat-12/">v12 (未支持)</a>
            <a href="../">parent</a>
        "#;
        assert_eq!(parse_tomcat_root_listing(html), vec![9, 10, 11]);
    }

    #[test]
    fn tomcat_branch_listing_filters_milestones_and_sorts() {
        let html = r#"
            <a href="v11.0.0-M1/">M1</a>
            <a href="v11.0.5/">x</a>
            <a href="v11.0.29/">y</a>
            <a href="v11.0.9/">z</a>
            <a href="KEYS">keys</a>
        "#;
        let entries = parse_tomcat_branch_listing(html).unwrap();
        let versions: Vec<&str> = entries.iter().map(|e| e.full_version.as_str()).collect();
        assert_eq!(versions, vec!["11.0.29", "11.0.9", "11.0.5"]);
        // feature_version = 版本首段（Apache 分支目录就是主版本号）
        assert!(entries.iter().all(|e| e.feature_version.as_deref() == Some("11")));
    }

    // ── Java 供应商解析 ─────────────────────────────────────────

    /// 回归：Java 信息源必须把 config 的 version_fallback_url 传下去（主源 404 时才能切换备源）
    #[test]
    fn adoptium_major_sources_include_configured_fallback() {
        // 主源来自 config，备源来自 config
        let src =
            adoptium_major_sources("https://mirror.example/info.json", Some("https://upstream.example/info.json"));
        assert_eq!(src.primary_url, "https://mirror.example/info.json");
        assert_eq!(src.secondary_url.as_deref(), Some("https://upstream.example/info.json"));

        // 主源为空 → 用内置 Adoptium；备源照常保留
        let src = adoptium_major_sources("", Some("https://upstream.example/info.json"));
        assert_eq!(src.primary_url, ADOPTIUM_INFO_URL);
        assert_eq!(src.secondary_url.as_deref(), Some("https://upstream.example/info.json"));

        // 备源为空/与主源相同 → 不设备源（避免重复请求同一 URL）
        assert!(adoptium_major_sources("https://a.example/x", None).secondary_url.is_none());
        assert!(
            adoptium_major_sources("https://a.example/x", Some("  "))
                .secondary_url
                .is_none()
        );
        assert!(
            adoptium_major_sources("https://a.example/x", Some("https://a.example/x"))
                .secondary_url
                .is_none()
        );
    }

    #[test]
    fn temurin_releases_skip_release_without_host_binary() {
        // 用当前宿主平台拼 fixture（解析器按宿主过滤）
        let os = detect_os_with(OsStyle::Adoptium);
        let arch = detect_arch_with(ArchStyle::Adoptium);
        let body = format!(
            r#"[
              {{"version_data": {{"semver": "21.0.13+9"}},
                "binaries": [{{"os": "{os}", "architecture": "{arch}", "package": {{"link": "https://example.com/jdk21.zip"}}}}]}},
              {{"version_data": {{"semver": "21.0.12+8"}},
                "binaries": [{{"os": "plan9", "architecture": "mips", "package": {{"link": "https://example.com/no.zip"}}}}]}}
            ]"#
        );
        let releases = parse_temurin_releases(&body).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].numeric, "21.0.13");
        assert_eq!(releases[0].download_url.as_deref(), Some("https://example.com/jdk21.zip"));
    }

    #[test]
    fn zulu_releases_filter_variants_and_musl_and_keep_newest_build() {
        let pkg = |name: &str, url: &str, ver: Vec<u32>| ZuluPackage {
            name: name.to_string(),
            download_url: url.to_string(),
            java_version: ver,
        };
        let all = vec![
            pkg("zulu21.46.19-ca-jdk21.0.9-win_x64.zip", "u1", vec![21, 0, 9, 0, 1]),
            pkg("zulu21.54.109-ca-jdk21.0.13.0.3-win_x64.zip", "u2", vec![21, 0, 13, 0, 3]),
            pkg("zulu21.50.19-ca-fx-jdk21.0.11-win_x64.zip", "fx", vec![21, 0, 11, 0, 1]),
            pkg("zulu21.50.19-ca-crac-jdk21.0.11-win_x64.zip", "crac", vec![21, 0, 11, 0, 1]),
            pkg("zulu21.52.15-ca-jdk21.0.12-win_x64.zip", "old", vec![21, 0, 12, 0, 1]),
            pkg("zulu21.52.16-ca-jdk21.0.12-win_x64.zip", "new", vec![21, 0, 12, 0, 2]),
            pkg(
                "zulu21.54.109-ca-jdk21.0.13-linux_musl_x64.tar.gz",
                "musl",
                vec![21, 0, 13, 0, 3],
            ),
        ];
        let out = zulu_releases(all, Some(21), false);
        let got: Vec<(&str, Option<&str>)> =
            out.iter().map(|r| (r.numeric.as_str(), r.download_url.as_deref())).collect();
        assert_eq!(
            got,
            vec![("21.0.13", Some("u2")), ("21.0.12", Some("new")), ("21.0.9", Some("u1"))]
        );
        // musl 宿主只留 musl 包
        let musl = zulu_releases(
            vec![pkg(
                "zulu21.54.109-ca-jdk21.0.13-linux_musl_x64.tar.gz",
                "musl",
                vec![21, 0, 13, 0, 3],
            )],
            None,
            true,
        );
        assert_eq!(musl.len(), 1);
    }

    #[test]
    fn corretto_tags_filter_non_numeric_and_build_platform_url() {
        let body = r#"[
          {"tag_name": "21.0.13.0.11", "prerelease": false, "draft": false, "assets": []},
          {"tag_name": "8.512.09.1", "prerelease": false, "draft": false, "assets": []},
          {"tag_name": "nightly", "prerelease": false, "draft": false, "assets": []},
          {"tag_name": "21.0.12.12.1", "prerelease": true, "draft": false, "assets": []}
        ]"#;
        let win = parse_corretto_tags(body, "windows", "x64", "zip").unwrap();
        assert_eq!(win.len(), 2);
        assert!(
            win[0]
                .download_url
                .as_deref()
                .unwrap()
                .ends_with("/21.0.13.0.11/amazon-corretto-21.0.13.0.11-windows-x64-jdk.zip")
        );
        let mac = parse_corretto_tags(body, "macos", "aarch64", "tar.gz").unwrap();
        assert!(mac[0].download_url.as_deref().unwrap().contains("-macosx-aarch64.tar.gz"));
        let linux = parse_corretto_tags(body, "linux", "x64", "tar.gz").unwrap();
        assert!(linux[0].download_url.as_deref().unwrap().contains("-linux-x64.tar.gz"));
    }

    #[test]
    fn liberica_releases_strip_build_and_keep_newest() {
        let body = r#"[
          {"version": "21.0.12+9", "downloadUrl": "old"},
          {"version": "21.0.13+11", "downloadUrl": "new"},
          {"version": "21.0.13+10", "downloadUrl": "older"}
        ]"#;
        let out = parse_liberica_releases(body).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].numeric, "21.0.13");
        assert_eq!(out[0].download_url.as_deref(), Some("new"));
    }
}
