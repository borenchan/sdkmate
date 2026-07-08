use crate::config::NetworkConfig;
use anyhow::{Context, Result, bail};
use indicatif::{HumanBytes, ProgressBar};
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use util::{detail, warning};

/// Build reqwest Client from NetworkConfig.
/// - User-Agent is required by GitHub API
/// - github_token (if configured) adds Authorization header,
///   increasing GitHub API rate limit from 60/hr to 5000/hr
pub fn build_reqwest_client(network: &NetworkConfig) -> Result<Client> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(network.connect_timeout as u64))
        .timeout(Duration::from_secs(600))
        .gzip(true)
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("sdkm/0.2");

    // Add GitHub token as default Authorization header if configured
    if let Some(token) = &network.github_token {
        let auth_value = format!("Bearer {}", token);
        let header_value = reqwest::header::HeaderValue::from_str(&auth_value).context("Invalid github_token value")?;
        builder = builder.default_headers([(reqwest::header::AUTHORIZATION, header_value)].into_iter().collect());
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

/// 异步下载文件到指定路径，支持 streaming + 进度条 + 断点续传。
///
/// 续传机制：若 dest_path 已有部分文件，发 `Range: bytes=<have>-` 请求；
/// 服务器返 206 → append 续传、进度条从 have 开始；
/// 服务器返 200（不支持 Range 或文件已变）→ 从头覆盖下载。
/// 同版本 tmp_dir 隔离保证只续传同 URL，不会跨版本拼接损坏。
pub async fn download_with_progress(client: &Client, url: &str, dest_path: &Path, pb: &ProgressBar) -> Result<()> {
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
    let mut resp = req.send()
        .await
        .context(format!("Failed to start download from {}", url))?;

    let status = resp.status();
    // 206 = 服务器支持续传；200 = 不支持 Range 或无部分文件，从头下
    let resume = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !resume && !status.is_success() {
        bail!("Download failed: HTTP {} from {}", status, url);
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
            pb.suspend(|| { detail!("Resuming download: {} already downloaded", HumanBytes(have)); });
        }
        tokio::fs::OpenOptions::new().append(true).open(dest_path)
            .await
            .context("Failed to open partial file for resume")?
    } else {
        if have > 0 {
            pb.suspend(|| { detail!("Server doesn't support resume, restarting download"); });
        }
        tokio::fs::File::create(dest_path)
            .await
            .context("Failed to create download file")?
    };
    // 128KB 写缓冲：攒满再 flush，减少写盘系统调用次数（reqwest chunk 通常 8-16KB，否则每块一次 syscall）
    let mut writer = tokio::io::BufWriter::with_capacity(128 * 1024, file);

    // 进度条对齐：续传从 have 开始，从头从 0
    pb.set_position(if resume { have } else { 0 });

    // 用 reqwest 的 chunk() 逐块读取（无需 StreamExt，省去 futures-util 依赖）
    while let Some(chunk) = resp.chunk().await.context("Error reading download stream")? {
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
    pb: &ProgressBar,
    max_retries: u32,
) -> Result<()> {
    for attempt in 0..max_retries {
        if attempt > 0 {
            pb.set_message(format!("Retry #{} downloading...", attempt));
        }

        match download_with_progress(client, url, dest_path, pb).await {
            Ok(_) => return Ok(()),
            Err(e) if attempt < max_retries - 1 => {
                // 保留部分文件供下次重试断点续传（不 remove_file）
                pb.suspend(|| {
                    warning!("Download attempt #{} failed: {}. Retrying (will resume)...", attempt + 1, e);
                });
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => bail!("Download failed after {} attempts: {}", max_retries, e),
        }
    }
    Ok(())
}
