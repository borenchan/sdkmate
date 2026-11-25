// 版本数据缓存 + 网络获取(无 SDK 语义,纯传输层)

use crate::config::NetworkConfig;
use crate::report::EventSink;
use anyhow::{Context, Result, bail};
use reqwest::Client;
use sdkm_util::consts::SDKM_CACHE_DIR;
use sdkm_util::path::get_sdkm_home;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use super::truncate;

/// 版本源配置(主/备)
pub struct VersionSource {
    pub primary_url: String,
    pub secondary_url: Option<String>,
}

// ─── 通用缓存(缓存优先 + TTL 过期) ─────────────────────────────

/// 缓存路径:<sdkm_home>/.cache/api/<sdk_name>.json
fn cache_path(sdk_name: &str) -> Result<PathBuf> {
    let dir = get_sdkm_home()?.join(SDKM_CACHE_DIR).join("api");
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

/// 最后一次兜底（无任何备援）时的尝试次数：1 次重试
const LAST_RESORT_ATTEMPTS: u32 = 2;

/// 单次请求等待响应头的上限（connect+TLS+响应头）。
/// connect_timeout 只管 TCP 建连，黑洞源可能在建连后长时间不发响应头；
/// 有候选/备源时不该为一个源等到 connect_timeout(默认 30s) 才切换。
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);

/// 缓存优先的版本数据获取,带 TTL 与 API 兜底。
///
/// 流程:查缓存 → 新鲜则立即返回 → 过期/缺失
///       → 主源候选链（有备援则每个只试 1 次，直接切）
///       → 备源候选链（多候选同样 1 次；单候选为最后兜底，允许 1 次重试）
///       → 写入缓存 → 返回。
///       主备均失败 → 返回过期缓存作为降级兜底 → bail。
#[allow(clippy::too_many_arguments)] // 传输层管线参数天然多（client/source/cache/label/ttl/network/sink），打包反而模糊
pub async fn fetch_version_data(
    client: &Client,
    source: &VersionSource,
    cache_key: &str,
    sdk_label: &str,
    headers: Option<HashMap<String, String>>,
    cache_ttl_secs: u64,
    network: &NetworkConfig,
    sink: &dyn EventSink,
) -> Result<String> {
    // 缓存优先:新鲜则立即返回,不发起网络请求
    if let Some(cached) = load_from_cache_if_fresh(cache_key, cache_ttl_secs) {
        return Ok(cached);
    }

    // 候选源链：GitHub 白名单域名展开为「镜像前缀 + 高校镜像 + 直连」，
    // 单个坏源不再吃掉整段重试时间（api.github.com 等非白名单 URL 只有一个候选）
    let primary_candidates = network.download_candidates(&source.primary_url);
    let secondary_candidates = source
        .secondary_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| network.download_candidates(s));

    // token 只注入 GH API 请求（client 不再全局带 Authorization，防镜像下载泄露给第三方）；
    // api.github.com 不在镜像白名单内，重写前后 URL 的"是否 GH API"判定等价
    // 外层主备策略：主源有备援（多候选 / 配置了备源）时只给 1 次机会，失败立刻切；
    // 备源组内多候选同样 1 次；只有单候选的最后兜底才允许 1 次重试
    let primary_attempts = if primary_candidates.len() > 1 || secondary_candidates.is_some() {
        1
    } else {
        LAST_RESORT_ATTEMPTS
    };
    let primary_result = try_candidates(
        client,
        &primary_candidates,
        headers.clone(),
        sdk_label,
        "primary",
        sink,
        network,
        primary_attempts,
    )
    .await;

    if let Ok(body) = primary_result {
        let _ = save_to_cache(cache_key, &body);
        return Ok(body);
    }

    // 主源失败原因打出（404 等确定性失败此前静默跳备源，用户看不到主源已失效）。
    // try_url_with_retry 的错误串自带 "[sdk] primary source ..." 前缀，这里剥掉避免重复
    let primary_err = primary_result.unwrap_err();
    let strip_prefix = |e: anyhow::Error, label: &str| -> String {
        e.to_string()
            .strip_prefix(&format!("[{}] ", label))
            .unwrap_or(&e.to_string())
            .to_string()
    };
    let primary_err_str = strip_prefix(primary_err, sdk_label);
    if let Some(secondary_candidates) = &secondary_candidates {
        sink.warning(format!(
            "[{}] primary source failed ({}), trying secondary source...",
            sdk_label,
            truncate(&primary_err_str, 160)
        ));
        let secondary_attempts = if secondary_candidates.len() > 1 {
            1
        } else {
            LAST_RESORT_ATTEMPTS
        };
        let secondary_result = try_candidates(
            client,
            secondary_candidates,
            headers,
            sdk_label,
            "secondary",
            sink,
            network,
            secondary_attempts,
        )
        .await;

        if let Ok(body) = secondary_result {
            let _ = save_to_cache(cache_key, &body);
            return Ok(body);
        }
        sink.warning(format!(
            "[{}] secondary source also failed ({}), falling back to stale cache if any",
            sdk_label,
            truncate(&strip_prefix(secondary_result.unwrap_err(), sdk_label), 160)
        ));
    } else {
        // 无备源配置时把主源错误带上，避免只剩笼统的 no cache 提示
        return Err(anyhow::anyhow!("[{}] {}", sdk_label, primary_err_str));
    }

    // 主备均失败 → 返回过期缓存作为降级兜底
    if let Some(cached) = load_from_cache_stale(cache_key) {
        sink.warning(format!(
            "[{sdk_label}] both primary and secondary sources failed, using stale local cache (may be outdated)"
        ));
        return Ok(cached);
    }

    bail!(
        "[{}] all version sources failed with no local cache available. Check your network or configure a mirror URL",
        sdk_label
    );
}

/// GH API 请求附加 Authorization header（未配置 token 或非 GH API URL 则原样返回）
///
/// token 不再走 client 全局 header（镜像开启时下载请求会把 token 发给第三方代理），
/// 由本函数按 URL 判定精确注入。
fn with_github_token_if_api(
    mut headers: Option<HashMap<String, String>>,
    url: &str,
    network: &NetworkConfig,
) -> Option<HashMap<String, String>> {
    let Some(token) = network.github_token.as_deref().filter(|t| !t.is_empty()) else {
        return headers;
    };
    if !url.contains("api.github.com/") {
        return headers;
    }
    headers
        .get_or_insert_with(HashMap::new)
        .insert("Authorization".to_string(), format!("Bearer {}", token));
    headers
}

/// 按候选顺序逐个尝试；`attempts` 由调用方按「有无备援」决定：
/// 有备援传 1（下一个源就是重试），最后兜底才传 LAST_RESORT_ATTEMPTS
#[allow(clippy::too_many_arguments)] // 传输层管线参数天然多（client/候选/头/标签/sink/network/预算），打包反而模糊
async fn try_candidates(
    client: &Client,
    candidates: &[String],
    headers: Option<HashMap<String, String>>,
    sdk_label: &str,
    source_label: &str,
    sink: &dyn EventSink,
    network: &NetworkConfig,
    attempts: u32,
) -> Result<String> {
    let mut last_err: Option<anyhow::Error> = None;
    for (i, url) in candidates.iter().enumerate() {
        if i > 0 {
            sink.warning(format!(
                "[{}] {} source unreachable, trying next mirror...",
                sdk_label, source_label
            ));
        }
        let hdrs = with_github_token_if_api(headers.clone(), url, network);
        match try_url_with_retry(client, url, hdrs, sdk_label, source_label, sink, attempts).await {
            Ok(body) => return Ok(body),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err
        .unwrap_or_else(|| anyhow::anyhow!("[{}] {} source has no candidates configured", sdk_label, source_label)))
}

/// 对单个 URL 进行重试(5xx/网络错误重试,429/403 不重试直接返回错误)
async fn try_url_with_retry(
    client: &Client,
    url: &str,
    headers: Option<HashMap<String, String>>,
    sdk_label: &str,
    source_label: &str,
    sink: &dyn EventSink,
    attempts_budget: u32,
) -> Result<String> {
    for attempt in 0..attempts_budget {
        let mut req = client.get(url);
        if let Some(hdrs) = &headers {
            for (k, v) in hdrs {
                req = req.header(k.as_str(), v.as_str());
            }
        }
        let resp = match tokio::time::timeout(RESPONSE_TIMEOUT, req.send()).await {
            Ok(resp) => resp,
            Err(_) => {
                // 响应头超时按网络错误处理：有备援时 attempts=1 直接返回让上层切源
                let msg = format!("no response within {}s", RESPONSE_TIMEOUT.as_secs());
                if attempt + 1 < attempts_budget {
                    let delay = Duration::from_secs((attempt + 1) as u64);
                    sink.warning(format!(
                        "[{}] {} source {}, retrying in {}s... (attempt {}/{})",
                        sdk_label,
                        source_label,
                        msg,
                        delay.as_secs(),
                        attempt + 1,
                        attempts_budget
                    ));
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "[{}] {} source {} after {} attempts",
                    sdk_label,
                    source_label,
                    msg,
                    attempts_budget
                ));
            }
        };

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
                    if attempt + 1 < attempts_budget {
                        let delay = Duration::from_secs((attempt + 1) as u64);
                        sink.warning(format!(
                            "[{}] {} source server error (HTTP {}), retrying in {}s... (attempt {}/{})",
                            sdk_label,
                            source_label,
                            status,
                            delay.as_secs(),
                            attempt + 1,
                            attempts_budget
                        ));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "[{}] {} source server error (HTTP {}) after {} attempts",
                        sdk_label,
                        source_label,
                        status,
                        attempts_budget
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
                if attempt + 1 < attempts_budget {
                    let delay = Duration::from_secs((attempt + 1) as u64);
                    sink.warning(format!(
                        "[{}] {} source network error, retrying in {}s... (attempt {}/{}): {}",
                        sdk_label,
                        source_label,
                        delay.as_secs(),
                        attempt + 1,
                        attempts_budget,
                        truncate(&e.to_string(), 100)
                    ));
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "[{}] {} source network error after {} attempts: {}",
                    sdk_label,
                    source_label,
                    attempts_budget,
                    truncate(&e.to_string(), 200)
                ));
            }
        }
    }

    bail!("[{}] {} source all retries exhausted", sdk_label, source_label);
}
