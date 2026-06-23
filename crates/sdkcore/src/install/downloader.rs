use crate::config::NetworkConfig;
use anyhow::{Context, Result, bail};
use indicatif::ProgressBar;
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use util::warning;

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

/// 异步下载文件到指定路径，支持 streaming + 进度条
pub async fn download_with_progress(client: &Client, url: &str, dest_path: &Path, pb: &ProgressBar) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to start download from {}", url))?;

    if !resp.status().is_success() {
        bail!("Download failed: HTTP {} from {}", resp.status(), url)
    }

    // content_length 可能为 0（redirect URL 不提供 Content-Length）
    // 只在有值时设置进度条总长度
    let total_size = resp.content_length().unwrap_or(0);
    if total_size > 0 {
        pb.set_length(total_size);
    }

    let parent_dir = dest_path.parent().context("Download destination has no parent directory")?;
    tokio::fs::create_dir_all(parent_dir)
        .await
        .context("Failed to create download destination directory")?;

    let mut file = tokio::fs::File::create(dest_path)
        .await
        .context("Failed to create download file")?;

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading download stream")?;
        pb.inc(chunk.len() as u64);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .context("Error writing download chunk to file")?;
    }

    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .context("Error flushing download file")?;

    Ok(())
}

/// 带重试的下载（最多 max_retries 次）
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
            pb.reset();
        }

        match download_with_progress(client, url, dest_path, pb).await {
            Ok(_) => return Ok(()),
            Err(e) if attempt < max_retries - 1 => {
                let _ = tokio::fs::remove_file(dest_path).await;
                pb.suspend(|| {
                    warning!("Download attempt #{} failed: {}. Retrying...", attempt + 1, e);
                });
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            Err(e) => bail!("Download failed after {} attempts: {}", max_retries, e),
        }
    }
    Ok(())
}
