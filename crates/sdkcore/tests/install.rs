use indicatif::{ProgressBar, ProgressDrawTarget};
use sdkcore::install::downloader::download_with_progress;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// 起一个本地 HTTP/1.1 server 返回固定 body，返回监听 URL
fn serve(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf); // 读并丢弃请求头
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    url
}

/// 核心链路：resp.chunk() 逐块读取 + 128KB BufWriter 写入 + flush 后文件内容完整。
/// 覆盖小 body（1B）与 >128KB body（触发 BufWriter 多次 flush 的边界）
#[test]
fn download_writes_full_body() {
    let cases: [Vec<u8>; 2] = [vec![b'a'; 1], vec![b'b'; 200_000]];
    let client = reqwest::Client::new();
    for (i, body) in cases.into_iter().enumerate() {
        let url = serve(body.clone());
        let dest = env::temp_dir().join(format!("sdkm_dl_test_{}.bin", i));
        let pb = ProgressBar::new(0);
        pb.set_draw_target(ProgressDrawTarget::hidden());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(download_with_progress(&client, &url, &dest, &pb)).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        let _ = fs::remove_file(&dest);
    }
}

/// 起一个支持 Range 请求的本地 HTTP/1.1 server：解析 `Range: bytes=START-`，返 206 + 剩余 body
fn serve_range(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            let start = req
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("range: bytes="))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|s| s.trim().split('-').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            if start >= body.len() {
                let head = "HTTP/1.1 416 Range Not Satisfiable\r\n\r\n";
                let _ = stream.write_all(head.as_bytes());
                return;
            }
            let remaining = &body[start..];
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n\r\n",
                remaining.len(),
                start,
                body.len() - 1,
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(remaining);
        }
    });
    url
}

/// 断点续传：预写前半文件，server 返 206，验证 append 后文件完整
#[test]
fn download_resumes_partial() {
    let body = vec![b'c'; 200_000];
    let url = serve_range(body.clone());
    let client = reqwest::Client::new();
    let dest = env::temp_dir().join("sdkm_dl_resume.bin");
    fs::write(&dest, &body[..100_000]).unwrap(); // 预写前 100KB
    let pb = ProgressBar::new(0);
    pb.set_draw_target(ProgressDrawTarget::hidden());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(download_with_progress(&client, &url, &dest, &pb)).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), body);
    let _ = fs::remove_file(&dest);
}

/// 服务器不支持 Range（返 200 全量）时，从头覆盖已有部分文件
#[test]
fn download_restarts_when_no_range_support() {
    let body = vec![b'd'; 200_000];
    let url = serve(body.clone()); // serve 只返 200，不解析 Range
    let client = reqwest::Client::new();
    let dest = env::temp_dir().join("sdkm_dl_norange.bin");
    fs::write(&dest, b"STALE PARTIAL CONTENT").unwrap(); // 预写垃圾
    let pb = ProgressBar::new(0);
    pb.set_draw_target(ProgressDrawTarget::hidden());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(download_with_progress(&client, &url, &dest, &pb)).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), body);
    let _ = fs::remove_file(&dest);
}
