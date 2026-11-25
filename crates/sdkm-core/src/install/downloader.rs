use crate::config::NetworkConfig;
use crate::report::{EventSink, ProgressHandle};
use anyhow::{Context, Result, bail};
use reqwest::Client;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// 首响应等待上限（connect+TLS+响应头）：connect_timeout 默认 30s，
/// 黑洞源会把一次尝试拖到 30s，这里统一在发送阶段快速判死
const FIRST_RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);
/// 首字节等待上限：坏源常常接受 TCP 后不发数据，30s 会让用户长时间看不到进度
const FIRST_CHUNK_TIMEOUT: Duration = Duration::from_secs(8);
/// 传输中两段数据之间的最大静默时间
const STALL_TIMEOUT: Duration = Duration::from_secs(15);
/// 低速判定窗口与窗口内最低字节数（60s 内不足 300KB ≈ 均速 <5KB/s，视为假活连接）
const LOW_SPEED_WINDOW: Duration = Duration::from_secs(60);
const LOW_SPEED_MIN_BYTES: u64 = 300 * 1024;
/// 候选源健康探测单请求上限（探测只为排序，不值得等太久）
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

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
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("sdkm/0.2");

    if gzip {
        // API 请求（版本 JSON 等）：用读空闲超时而不是总超时——GH releases 单页可达数 MB，
        // 慢速链路需要几十秒传完；坏连接只要 15s 无数据就失败，不会挂到天荒地老。
        builder = builder.gzip(true).read_timeout(Duration::from_secs(15));
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

/// 卡死/假活连接：同一 URL 重试无意义，直接切下一候选源
#[derive(Debug)]
struct StalledDownload(String);

impl std::fmt::Display for StalledDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StalledDownload {}

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
    // 首响应单独限时：connect_timeout 只管 TCP 建连，TLS/等待响应头仍可能长时间无进展
    let mut resp = match tokio::time::timeout(FIRST_RESPONSE_TIMEOUT, req.send()).await {
        Ok(r) => r.context(format!("Failed to start download from {}", url))?,
        Err(_) => {
            return Err(anyhow::Error::new(StalledDownload(format!(
                "No response within {}s from {}",
                FIRST_RESPONSE_TIMEOUT.as_secs(),
                url
            ))));
        }
    };

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

    // 用 reqwest 的 chunk() 逐块读取（无需 StreamExt，省去 futures-util 依赖）。
    // 超时分两档：首字节 8s（死源往往接受 TCP 后零数据），之后静默 15s 判卡死；
    // 另加低速窗口兜底，防 1B/秒 的假活连接把大文件拖到天荒地老。
    let mut first = true;
    let mut win_start = Instant::now();
    let mut win_bytes = 0u64;
    loop {
        let wait = if first { FIRST_CHUNK_TIMEOUT } else { STALL_TIMEOUT };
        let chunk = match tokio::time::timeout(wait, resp.chunk()).await {
            Ok(chunk) => chunk.context("Error reading download stream")?,
            Err(_) => {
                return Err(anyhow::Error::new(StalledDownload(format!(
                    "Download stalled: no data for {}s from {}",
                    wait.as_secs(),
                    url
                ))));
            }
        };
        let Some(chunk) = chunk else { break };
        first = false;
        if pb.is_cancelled() {
            bail!("install cancelled");
        }
        pb.inc(chunk.len() as u64);
        tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk)
            .await
            .context("Error writing download chunk to file")?;

        // 低速窗口：60s 内累计不足 300KB 判为假活连接（正常慢速源仍可通过）
        win_bytes += chunk.len() as u64;
        if win_start.elapsed() >= LOW_SPEED_WINDOW {
            if win_bytes < LOW_SPEED_MIN_BYTES {
                return Err(anyhow::Error::new(StalledDownload(format!(
                    "Download too slow: {} in {}s from {}",
                    human_bytes(win_bytes),
                    LOW_SPEED_WINDOW.as_secs(),
                    url
                ))));
            }
            win_start = Instant::now();
            win_bytes = 0;
        }
    }

    tokio::io::AsyncWriteExt::flush(&mut writer)
        .await
        .context("Error flushing download file")?;

    Ok(())
}

/// 多候选源下载：先并行探测（Range 0-64KB）按响应快慢排序，再逐源尝试。
/// `attempts` 由调用方决定：有备援（其他候选/备源）传 1，失败立即切换；
/// 仅当这是最后一个兜底源时才传 2（允许 1 次重试）。卡死/4xx 永不重试。
/// 返回实际成功的 URL。
///
/// 候选通常指向同一资产（镜像前缀、直连、国内镜像），dest_path 不变：
/// 切源后靠 Range 续传，不重复下载已完成部分。
pub async fn download_best(
    client: &Client,
    candidates: &[String],
    dest_path: &Path,
    pb: &dyn ProgressHandle,
    sink: &dyn EventSink,
    attempts: u32,
) -> Result<String> {
    let mut order: Vec<usize> = if candidates.len() > 1 {
        probe_candidates(client, candidates).await
    } else {
        Vec::new()
    };
    // 探测全部失败时不放弃：仍按配置顺序尝试（探测可能误伤，下载本身才是最终判定）
    if order.is_empty() {
        order = (0..candidates.len()).collect();
    } else if order.len() < candidates.len() {
        // 探测成功的排前面，未响应的保底排后面
        let fillers: Vec<usize> = (0..candidates.len()).filter(|i| !order.contains(i)).collect();
        order.extend(fillers);
    }

    let mut failed_hosts: Vec<String> = Vec::new();
    for (pos, idx) in order.iter().enumerate() {
        let url = &candidates[*idx];
        pb.set_message(format!("Source: {}", url_host(url)));
        match download_from_candidate(client, url, dest_path, pb, sink, attempts).await {
            Ok(()) => {
                sink.detail(format!("Downloaded from {}", url_host(url)));
                return Ok(url.clone());
            }
            Err(e) => {
                let host = url_host(url);
                // 失败与切换合并为一条告警；末尾汇总只列失败源，
                // 避免同一段错误在"逐源告警 + 最终错误"里打印两遍
                let switch_hint = order
                    .get(pos + 1)
                    .map(|next| format!("; switching to {}", url_host(&candidates[*next])))
                    .unwrap_or_default();
                sink.warning(format!("Source {host} failed: {e:#}{switch_hint}"));
                failed_hosts.push(host);
                // 4xx/卡死之后可能已有半截文件，续传给下一源；从头失败的源不清文件
            }
        }
    }
    bail!(
        "All {} download sources failed: {}",
        failed_hosts.len(),
        failed_hosts.join(", ")
    );
}

/// 单候选源下载：`attempts` 次机会；卡死/4xx 立即返回让上层切源
async fn download_from_candidate(
    client: &Client,
    url: &str,
    dest_path: &Path,
    pb: &dyn ProgressHandle,
    sink: &dyn EventSink,
    attempts: u32,
) -> Result<()> {
    for attempt in 0..attempts {
        if pb.is_cancelled() {
            bail!("install cancelled");
        }
        if attempt > 0 {
            pb.set_message(format!("Retry #{} from {}...", attempt, url_host(url)));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        match download_with_progress(client, url, dest_path, pb, sink).await {
            Ok(()) => return Ok(()),
            Err(e) if e.downcast_ref::<NonRetryableHttpError>().is_some() => return Err(e),
            Err(e) if e.downcast_ref::<StalledDownload>().is_some() => return Err(e),
            Err(e) if attempt + 1 >= attempts => return Err(e),
            Err(e) => sink.warning(format!(
                "Download attempt #{} failed: {}. Retrying (will resume)...",
                attempt + 1,
                e
            )),
        }
    }
    unreachable!()
}

/// 并行探测候选源健康度：Range 0-64KB，读完首块，按「响应 + 首块传输」耗时升序返回可用下标。
/// 只测 TTFB 会选中"响应快但传输慢"的源，因此探测包带一点数据量。
/// 探测同时预热连接池，后续正式下载可直接复用 TLS 连接。
async fn probe_candidates(client: &Client, candidates: &[String]) -> Vec<usize> {
    let mut set = JoinSet::new();
    for (idx, url) in candidates.iter().enumerate() {
        let client = client.clone();
        let url = url.clone();
        set.spawn(async move {
            let start = Instant::now();
            let ok = match client
                .get(&url)
                .header(reqwest::header::RANGE, "bytes=0-65535")
                .timeout(PROBE_TIMEOUT)
                .send()
                .await
            {
                Ok(mut resp) if resp.status().is_success() => {
                    // 读首个 chunk 纳入耗时；读完即丢弃剩余 body
                    let _ = resp.chunk().await;
                    true
                }
                _ => false,
            };
            (idx, ok, start.elapsed())
        });
    }

    let mut alive = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok((idx, true, elapsed)) = joined {
            alive.push((idx, elapsed));
        }
    }
    alive.sort_by_key(|(_, elapsed)| *elapsed);
    alive.into_iter().map(|(idx, _)| idx).collect()
}

/// URL 展示用主机名（解析失败时回退原串，截断避免刷屏）
fn url_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
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
