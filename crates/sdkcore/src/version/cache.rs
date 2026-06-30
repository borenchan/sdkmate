// 版本数据缓存 + 网络获取(无 SDK 语义,纯传输层)

use anyhow::{Context, Result, bail};
use reqwest::Client;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use util::path::get_sdkm_home;
use util::sdk_resources::SdkSourceConfig;
use util::warning;

use super::truncate;

/// 版本源配置(主/备)
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

// ─── 通用缓存(缓存优先 + TTL 过期) ─────────────────────────────

/// 缓存路径:<sdkm_home>/.cache/api/<sdk_name>.json
fn cache_path(sdk_name: &str) -> Result<std::path::PathBuf> {
    let dir = get_sdkm_home()?.join(".cache").join("api");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.json", sdk_name)))
}

fn save_to_cache(sdk_name: &str, body: &str) -> Result<()> {
    fs::write(cache_path(sdk_name)?, body)?;
    Ok(())
}

/// 仅当缓存新鲜(在 TTL 内)时加载。
/// 以文件修改时间作为隐式时间戳——无需解析额外元数据。
fn load_from_cache_if_fresh(sdk_name: &str, ttl_secs: u64) -> Option<String> {
    let path = cache_path(sdk_name).ok().filter(|p| p.exists())?;
    let mtime = fs::metadata(&path).ok()?.modified().ok()?;
    let age = mtime.elapsed().ok()?.as_secs();
    if age <= ttl_secs {
        fs::read_to_string(&path).ok()
    } else {
        None
    }
}

/// 不考虑时效直接加载缓存(网络失败时的降级兜底)。
fn load_from_cache_stale(sdk_name: &str) -> Option<String> {
    cache_path(sdk_name)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
}

// ─── 通用网络请求:主备切换 + 重试 + 缓存兜底 ──────────────────

/// 最大重试次数(仅对 5xx / 网络错误重试,429/403 不重试)
const MAX_RETRIES: u32 = 3;

/// 缓存优先的版本数据获取,带 TTL 与 API 兜底。
///
/// 流程:查缓存 → 新鲜则立即返回 → 过期/缺失
///       → 主源(重试 3 次)→ 备源(重试 3 次)
///       → 写入缓存 → 返回。
///       主备均失败 → 返回过期缓存作为降级兜底 → bail。
pub async fn fetch_version_data(
    client: &Client,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    headers: Option<HashMap<String, String>>,
    cache_ttl_secs: u64,
) -> Result<String> {
    // 缓存优先:新鲜则立即返回,不发起网络请求
    if let Some(cached) = load_from_cache_if_fresh(cache_key, cache_ttl_secs) {
        return Ok(cached);
    }

    // 缓存过期或缺失 → 走 API
    let primary_result = try_url_with_retry(client, &source.primary_url, headers.clone(), sdk_label, "primary").await;

    if let Ok(body) = primary_result {
        let _ = save_to_cache(cache_key, &body);
        return Ok(body);
    }

    // 主源失败 → 试备源
    if let Some(secondary_url) = &source.secondary_url {
        if !secondary_url.is_empty() {
            let secondary_result =
                try_url_with_retry(client, secondary_url, headers.clone(), sdk_label, "secondary").await;

            if let Ok(body) = secondary_result {
                let _ = save_to_cache(cache_key, &body);
                return Ok(body);
            }
        }
    }

    // 主备均失败 → 返回过期缓存作为降级兜底
    if let Some(cached) = load_from_cache_stale(cache_key) {
        warning!(
            "[{}] both primary and secondary sources failed, using stale local cache (may be outdated)",
            sdk_label
        );
        return Ok(cached);
    }

    bail!(
        "[{}] all version sources failed with no local cache available. Check your network or configure a mirror URL",
        sdk_label
    );
}

/// 对单个 URL 进行重试(5xx/网络错误重试,429/403 不重试直接返回错误)
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
                return resp
                    .text()
                    .await
                    .context(format!("[{}] failed to read {} source response", sdk_label, source_label));
            }
            Ok(resp) => {
                let status = resp.status();
                let status_code = status.as_u16();

                // 速率限制 → 不重试,直接返回错误(让上层切备源或走缓存)
                if status_code == 403 || status_code == 429 {
                    return Err(anyhow::anyhow!(
                        "[{}] {} source rate-limited (HTTP {})",
                        sdk_label,
                        source_label,
                        status
                    ));
                }

                // 5xx 服务器错误 → 重试
                if status.is_server_error() {
                    if attempt < MAX_RETRIES - 1 {
                        let delay = Duration::from_secs((attempt + 1) as u64);
                        warning!(
                            "[{}] {} source server error (HTTP {}), retrying in {}s... (attempt {}/{})",
                            sdk_label,
                            source_label,
                            status,
                            delay.as_secs(),
                            attempt + 1,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "[{}] {} source server error (HTTP {}) after {} retries",
                        sdk_label,
                        source_label,
                        status,
                        MAX_RETRIES
                    ));
                }

                // 其他 HTTP 错误 → 不重试
                return Err(anyhow::anyhow!(
                    "[{}] {} source returned HTTP {}",
                    sdk_label,
                    source_label,
                    status
                ));
            }
            Err(e) => {
                // 网络错误 → 重试
                if attempt < MAX_RETRIES - 1 {
                    let delay = Duration::from_secs((attempt + 1) as u64);
                    warning!(
                        "[{}] {} source network error, retrying in {}s... (attempt {}/{}): {}",
                        sdk_label,
                        source_label,
                        delay.as_secs(),
                        attempt + 1,
                        MAX_RETRIES,
                        truncate(&e.to_string(), 100)
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "[{}] {} source network error after {} retries: {}",
                    sdk_label,
                    source_label,
                    MAX_RETRIES,
                    truncate(&e.to_string(), 200)
                ));
            }
        }
    }

    bail!("[{}] {} source all retries exhausted", sdk_label, source_label);
}
