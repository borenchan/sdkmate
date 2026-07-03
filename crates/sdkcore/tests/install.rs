use indicatif::{ProgressBar, ProgressDrawTarget};
use sdkcore::install::downloader::download_with_progress;
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
        let dest = std::env::temp_dir().join(format!("sdkm_dl_test_{}.bin", i));
        let pb = ProgressBar::new(0);
        pb.set_draw_target(ProgressDrawTarget::hidden());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(download_with_progress(&client, &url, &dest, &pb)).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        let _ = std::fs::remove_file(&dest);
    }
}
