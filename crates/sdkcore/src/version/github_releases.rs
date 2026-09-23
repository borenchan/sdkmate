// 标准 B：GitHub Releases 版本发现引擎
//
// version_url 指向 GH releases API 即自动走此引擎（is_github_releases_url 判定）：
//   1. 解析 releases JSON，滤掉 prerelease/draft
//   2. tag 剥前缀得版本号（bun-v1.4.2 / rust-v0.153.4 / v2.9.6 / 0.12.11 → 剥到首个数字）
//   3. 从 assets 中挑选当前平台压缩包（asset_selector 打分），直链填入 VersionEntry.download_url
//   4. 无匹配资产的 release 仍输出版本条目（download_url=None），install 侧自然落到 download_url 模板
//
// 该引擎无任何 per-SDK 逻辑：接入新工具 = config 加一条 version_url。

use anyhow::{Context, Result};
use serde::Deserialize;

use super::asset_selector::{AssetCandidate, pick_asset};
use super::discovery::VersionEntry;

/// 判定 version_url 是否指向 GitHub Releases API（引擎分发依据）
pub fn is_github_releases_url(url: &str) -> bool {
    url.contains("api.github.com/repos/")
}

/// version_url 若为 GH API，自动补 per_page=100（GH 默认 30 条，高频发版工具不够用）
pub fn ensure_per_page(url: &str) -> String {
    if !is_github_releases_url(url) || url.contains("per_page=") {
        return url.to_string();
    }
    if url.contains('?') {
        format!("{}&per_page=100", url)
    } else {
        format!("{}?per_page=100", url)
    }
}

/// GitHub Releases API 响应的 Accept header（统一注入点，install/list 共用）
pub fn github_api_headers() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([("Accept".to_string(), "application/vnd.github+json".to_string())])
}

// ─── GH Releases JSON 结构 ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

// ─── 解析实现 ───────────────────────────────────────────────────

/// 解析 GH releases JSON 为版本条目列表
///
/// `asset_prefix`：资产前缀门（通常取主可执行文件名，如 bun/claude/gh）
pub fn parse_releases(body: &str, asset_prefix: &str) -> Result<Vec<VersionEntry>> {
    let releases: Vec<GitHubRelease> =
        serde_json::from_str(body).context("[GitHub Releases] failed to parse releases JSON")?;

    let mut entries: Vec<VersionEntry> = Vec::new();
    for release in &releases {
        // 预发布/草稿一律跳过（codex 的 alpha、helm 的 rc 靠此过滤）
        if release.prerelease || release.draft {
            continue;
        }
        // tag 剥前缀得版本号；剥完必须以数字开头（垃圾 tag 在此被丢弃）
        let version = strip_tag_prefix(&release.tag_name);
        let Some(version) = version else { continue };

        // 平台资产挑选（命中则填直链，未命中仍保留条目——install 落到模板）
        let candidates: Vec<AssetCandidate> = release
            .assets
            .iter()
            .map(|a| AssetCandidate {
                name: a.name.as_str(),
                download_url: a.browser_download_url.as_str(),
            })
            .collect();
        let direct_url = pick_asset(&candidates, asset_prefix).map(|c| c.download_url.to_string());

        let feature_version = version.split('.').next().map(|s| s.to_string());
        entries.push(VersionEntry {
            full_version: version,
            feature_version,
            release_tag: Some(release.tag_name.clone()),
            download_url: direct_url,
        });
    }

    // 按版本号从高到低（现有通用排序口径）
    entries.sort_by(|a, b| {
        let va: Vec<u32> = a.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        let vb: Vec<u32> = b.full_version.split('.').filter_map(|s| s.parse().ok()).collect();
        vb.cmp(&va)
    });
    Ok(entries)
}

/// tag 剥前缀：跳过开头直到首个 ASCII 数字；剩余必须含 '.' 才视为版本号
fn strip_tag_prefix(tag: &str) -> Option<String> {
    let version = tag.trim_start_matches(|c: char| !c.is_ascii_digit());
    if version.is_empty() || !version.contains('.') {
        return None;
    }
    Some(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_prefix_stripping_all_shapes() {
        assert_eq!(strip_tag_prefix("bun-v1.4.2").as_deref(), Some("1.4.2"));
        assert_eq!(strip_tag_prefix("rust-v0.153.4").as_deref(), Some("0.153.4"));
        assert_eq!(strip_tag_prefix("v2.9.6").as_deref(), Some("2.9.6"));
        assert_eq!(strip_tag_prefix("0.12.11").as_deref(), Some("0.12.11"));
        assert_eq!(strip_tag_prefix("v4.2.4").as_deref(), Some("4.2.4"));
        // 非 tag 垃圾：无数字 / 无点 → 丢弃
        assert_eq!(strip_tag_prefix("merge_analyzer_branch"), None);
        assert_eq!(strip_tag_prefix("latest"), None);
    }

    #[test]
    fn url_detection_and_per_page() {
        assert!(is_github_releases_url("https://api.github.com/repos/oven-sh/bun/releases"));
        assert!(!is_github_releases_url("https://nodejs.org/dist/index.json"));
        assert!(!is_github_releases_url("https://services.gradle.org/versions/all"));

        // per_page 自动追加：无 query 用 ?，有 query 用 &，已含则不动
        assert_eq!(
            ensure_per_page("https://api.github.com/repos/a/b/releases"),
            "https://api.github.com/repos/a/b/releases?per_page=100"
        );
        assert_eq!(
            ensure_per_page("https://api.github.com/repos/a/b/releases?draft=true"),
            "https://api.github.com/repos/a/b/releases?draft=true&per_page=100"
        );
        assert_eq!(
            ensure_per_page("https://api.github.com/repos/a/b/releases?per_page=50"),
            "https://api.github.com/repos/a/b/releases?per_page=50"
        );
        // 非 GH URL 不动
        assert_eq!(ensure_per_page("https://go.dev/dl/?mode=json"), "https://go.dev/dl/?mode=json");
    }

    const BUN_BODY: &str = r#"[
      {
        "tag_name": "bun-v1.4.2",
        "prerelease": false,
        "draft": false,
        "assets": [
          {"name": "bun-windows-x64-baseline.zip", "browser_download_url": "https://x/baseline.zip"},
          {"name": "bun-windows-x64.zip", "browser_download_url": "https://x/plain.zip"},
          {"name": "bun-linux-x64-musl.zip", "browser_download_url": "https://x/musl.zip"}
        ]
      },
      {
        "tag_name": "bun-v1.4.1-rc.1",
        "prerelease": true,
        "draft": false,
        "assets": []
      },
      {
        "tag_name": "not-a-release-tag",
        "prerelease": false,
        "draft": false,
        "assets": []
      }
    ]"#;

    #[test]
    fn parse_bun_releases_end_to_end() {
        let entries = parse_releases(BUN_BODY, "bun").unwrap();
        // rc(prerelease) 与垃圾 tag 均被过滤
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.full_version, "1.4.2");
        assert_eq!(e.feature_version.as_deref(), Some("1"));
        assert_eq!(e.release_tag.as_deref(), Some("bun-v1.4.2"));
        // Windows 评测机：plain 版本胜 baseline
        let url = e.download_url.clone().unwrap_or_default();
        assert!(url.ends_with("plain.zip") || url.ends_with("musl.zip"), "got {}", url);
    }

    #[test]
    fn parse_helm_signature_only_release_falls_to_template() {
        // helm v4.x：GH release 仅签名文件，无二进制资产 → 条目保留、直链 None
        let body = r#"[{
          "tag_name": "v4.2.4",
          "prerelease": false,
          "draft": false,
          "assets": [
            {"name": "helm-v4.2.4-windows-amd64.zip.sha256", "browser_download_url": "https://x/sha"},
            {"name": "helm-v4.2.4-windows-amd64.zip.asc", "browser_download_url": "https://x/asc"}
          ]
        }]"#;
        let entries = parse_releases(body, "helm").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].full_version, "4.2.4");
        assert!(entries[0].download_url.is_none());
    }

    #[test]
    fn parse_empty_and_malformed() {
        assert!(parse_releases("[]", "bun").unwrap().is_empty());
        assert!(parse_releases("not json", "bun").is_err());
    }
}
