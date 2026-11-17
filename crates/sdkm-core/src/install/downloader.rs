use crate::config::NetworkConfig;
use crate::report::{EventSink, ProgressHandle};
use anyhow::{Context, Result, bail};
use reqwest::Client;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// 进程级 client 缓存（api / download 各一），按 network 配置指纹失效。
/// 前提：进程内 runtime 常驻（CLI OnceLock / GUI 共享 runtime）——连接池绑定创建它的
/// runtime，runtime 销毁则连接必死，因此只在两前端持有常驻 runtime 后启用。
type CachedClient = OnceLock<Mutex<Option<(u64, Client)>>>;
static API_CLIENT: CachedClient = OnceLock::new();
static DOWNLOAD_CLIENT: CachedClient = OnceLock::new();

/// proxy/SSL/超时 变更即视为不同 client；github_token 走请求头不影响传输层，不参与指纹
fn network_fingerprint(network: &NetworkConfig) -> u64 {
    let mut h = DefaultHasher::new();
    network.proxy.hash(&mut h);
    network.ssl_verify.hash(&mut h);
    network.connect_timeout.hash(&mut h);
    h.finish()
}

fn cached_client(slot: &CachedClient, network: &NetworkConfig, gzip: bool) -> Result<Client> {
    let mut guard = slot.get_or_init(Default::default).lock().unwrap();
    let fp = network_fingerprint(network);
    if let Some((f, c)) = guard.as_ref()
        && *f == fp
    {
        return Ok(c.clone());
    }
    let client = build_client(network, gzip)?;
    *guard = Some((fp, client.clone()));
    Ok(client)
}

/// Build reqwest Client from NetworkConfig.
/// User-Agent is required by GitHub API.
/// 注意：github_token 不在此全局注入（避免镜像开启时把 token 发给第三方代理），
/// 由各 GH API 请求点自行添加 Authorization header（github_api_headers 或请求级 header）。
pub fn build_reqwest_client(network: &NetworkConfig) -> Result<Client> {
    cached_client(&API_CLIENT, network, true)
}

/// 下载专用 client：禁 gzip。dl.google.com 等服务器对 Accept-Encoding: gzip 会真压缩，
/// tower-http 解码层同时剥掉 content-encoding 与 content-length 头 → content_length() 恒 None → 进度条失效。
/// 注意 reqwest 启用 gzip feature 后 client 默认就发 Accept-Encoding: gzip，
/// 仅"不调用 .gzip(true)"无效，必须显式 .no_gzip()。
/// 下载物本就是 zip/tar.gz，gzip 传输仅省 ~1.5% 还白耗 CPU；API 类小请求继续用 build_reqwest_client。
pub fn build_download_client(network: &NetworkConfig) -> Result<Client> {
    cached_client(&DOWNLOAD_CLIENT, network, false)
}

fn build_client(network: &NetworkConfig, gzip: bool) -> Result<Client> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(network.connect_timeout as u64))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("sdkm/0.2");

    if gzip {
        builder = builder.gzip(true);
    } else {
        builder = builder.no_gzip();
    }

    if let Some(proxy_url) = &network.proxy {
        let proxy = reqwest::Proxy::all(proxy_url).context(format!("Invalid proxy URL: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    }

    if !network.ssl_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder.build().context("Failed to build reqwest client")
}

/// 确定性 HTTP 失败（4xx 非限流类）：同一 URL 重试无意义，`download_with_retry` 直接放弃让备源接手
#[derive(Debug)]
struct NonRetryableHttpError(String);

impl std::fmt::Display for NonRetryableHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NonRetryableHttpError {}

/// 异步下载文件到指定路径，支持 streaming + 进度条 + 断点续传。
///
/// 续传机制：若 dest_path 已有部分文件，发 `Range: bytes=<have>-` 请求；
/// 服务器返 206 → append 续传、进度条从 have 开始；
/// 服务器返 200（不支持 Range 或文件已变）→ 从头覆盖下载。
/// 同版本 tmp_dir 隔离保证只续传同 URL，不会跨版本拼接损坏。
pub async fn download_with_progress(
    client: &Client,
    url: &str,
    dest_path: &Path,
    pb: &dyn ProgressHandle,
    sink: &dyn EventSink,
) -> Result<()> {
    // 断点续传：检查已下载部分文件大小
    let have: u64 = match tokio::fs::metadata(dest_path).await {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    // 已有部分则带 Range 请求续传
    let mut req = client.get(url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", have));
    }
    let mut resp = req.send().await.context(format!("Failed to start download from {}", url))?;

    let status = resp.status();
    // 206 = 服务器支持续传；200 = 不支持 Range 或无部分文件，从头下
    let resume = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !resume && !status.is_success() {
        let msg = format!("Download failed: HTTP {} from {}", status, url);
        // 4xx（除限流/超时类）是确定性失败：重试同一 URL 不会变好，直接交给备源
        // （典型场景：dlcdn 只有当前版本，装历史版本会 404 → 立即回退 archive）
        if status.is_client_error()
            && status != reqwest::StatusCode::TOO_MANY_REQUESTS
            && status != reqwest::StatusCode::REQUEST_TIMEOUT
        {
            return Err(anyhow::Error::new(NonRetryableHttpError(msg)));
        }
        bail!(msg);
    }

    // 206 时 Content-Length 是剩余字节数，总大小 = have + remaining；200 时是总大小
    let content_length = resp.content_length().unwrap_or(0);
    let total_size = if resume { have + content_length } else { content_length };
    if total_size > 0 {
        pb.set_length(total_size);
    }

    let parent_dir = dest_path.parent().context("Download destination has no parent directory")?;
    tokio::fs::create_dir_all(parent_dir)
        .await
        .context("Failed to create download destination directory")?;

    // 续传：append 打开（保留已有内容）；从头：create 覆盖
    let file = if resume {
        if have > 0 {
            sink.detail(format!("Resuming download: {} already downloaded", human_bytes(have)));
        }
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(dest_path)
            .await
            .context("Failed to open partial file for resume")?
    } else {
        if have > 0 {
            sink.detail("Server doesn't support resume, restarting download".to_string());
        }
        tokio::fs::File::create(dest_path)
            .await
            .context("Failed to create download file")?
    };
    // 128KB 写缓冲：攒满再 flush，减少写盘系统调用次数（reqwest chunk 通常 8-16KB，否则每块一次 syscall）
    let mut writer = tokio::io::BufWriter::with_capacity(128 * 1024, file);

    // 进度条对齐：续传从 have 开始，从头从 0
    pb.set_position(if resume { have } else { 0 });

    // 用 reqwest 的 chunk() 逐块读取（无需 StreamExt，省去 futures-util 依赖）；
    // 30s 无数据视为卡死（CDN 半开连接，实测 dlcdn 国内会停在几 MB），中止本次尝试触发重试/备源
    loop {
        let chunk = match tokio::time::timeout(Duration::from_secs(30), resp.chunk()).await {
            Ok(chunk) => chunk.context("Error reading download stream")?,
            Err(_) => bail!("Download stalled: no data for 30s from {}", url),
        };
        let Some(chunk) = chunk else { break };
        if pb.is_cancelled() {
            bail!("install cancelled");
        }
        pb.inc(chunk.len() as u64);
        tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk)
            .await
            .context("Error writing download chunk to file")?;
    }

    tokio::io::AsyncWriteExt::flush(&mut writer)
        .await
        .context("Error flushing download file")?;

    Ok(())
}

/// 带重试的下载（最多 max_retries 次）；失败时保留部分文件供下次重试断点续传
pub async fn download_with_retry(
    client: &Client,
    url: &str,
    dest_path: &Path,
    pb: &dyn ProgressHandle,
    sink: &dyn EventSink,
    max_retries: u32,
) -> Result<()> {
    for attempt in 0..max_retries {
        if pb.is_cancelled() {
            bail!("install cancelled");
        }
        if attempt > 0 {
            pb.set_message(format!("Retry #{} downloading...", attempt));
        }

        match download_with_progress(client, url, dest_path, pb, sink).await {
            Ok(_) => return Ok(()),
            // 4xx 确定性失败（如 dlcdn 装历史版本 404）：不重试，立即上抛交给备源
            Err(e) if e.downcast_ref::<NonRetryableHttpError>().is_some() => return Err(e),
            Err(e) if attempt < max_retries - 1 => {
                // 保留部分文件供下次重试断点续传（不 remove_file）
                sink.warning(format!(
                    "Download attempt #{} failed: {}. Retrying (will resume)...",
                    attempt + 1,
                    e
                ));
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => bail!("Download failed after {} attempts: {}", max_retries, e),
        }
    }
    Ok(())
}

/// 等价复刻 indicatif::HumanBytes（1024 进制：<1KiB 打整数 B，其余两位小数 + KiB/MiB/...），
/// sdkcore 去掉 indicatif 依赖后保持该提示字节不变
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut v = n as f64;
    if v < 1024.0 {
        return format!("{v:.0} B");
    }
    for unit in UNITS {
        v /= 1024.0;
        if v < 1024.0 || unit == UNITS[UNITS.len() - 1] {
            return format!("{v:.2} {unit}");
        }
    }
    unreachable!()
}
