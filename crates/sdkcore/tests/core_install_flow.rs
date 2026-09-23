// 核心下载安装链路集成测试（改核心下载逻辑必跑）
//
// 覆盖两种接入标准，全部用本地 HTTP server mock，无真实网络依赖、CI 可跑：
//   - 标准 B（GitHub Releases）：version_url 为 GH API 形态 URL（指向本地 server）→ 引擎解析
//     releases JSON → 平台资产直链 → 下载解压（bun 真实形态建模）
//   - 标准 A（模板直链）：version_url 返回版本列表 JSON + download_url 占位符模板 → 下载解压
//
// 断言：install 成功 → 版本目录布局正确 → switch 符号链接指向 → current_version 落盘。
//
// 运行：cargo test -p sdkcore --test core_install_flow
// （SDKM_HOME 全局态用互斥锁串行，参考 uninstall.rs 惯例）

use anyhow::Result;
use sdkcore::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkcore::env::EnvOperation;
use sdkcore::link::symlink::read_symlink_target;
use sdkcore::manager::SdkManager;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};
use std::thread;

// 串行锁：SDKM_HOME 全局环境变量，并行会竞态
static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

// ─── 本地 HTTP server：按路径分发多资源 ─────────────────────────

/// 起 HTTP server，按精确路径返回 body（GH 引擎追加的 per_page 查询串按前缀兜底）
fn serve_routes(routes: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let map: HashMap<String, Vec<u8>> = routes.into_iter().map(|(p, b)| (p.to_string(), b)).collect();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let req = String::from_utf8_lossy(&buf);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let bare = path.split('?').next().unwrap_or("").to_string();
            let matched = map.get(&bare).map(|b| b.as_slice());
            let head = match matched {
                Some(body) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                ),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = stream.write_all(head.as_bytes());
            if let Some(body) = matched {
                let _ = stream.write_all(body);
            }
        }
    });
    base
}

/// 构造最小 GH Releases JSON（一个 stable release：当前平台资产 + 另一平台资产）
fn gh_releases_body(base: &str, asset_win: &str, asset_unix: &str, version_tag: &str) -> String {
    format!(
        r#"[{{"tag_name":"{tag}","prerelease":false,"draft":false,"assets":[{{"name":"{win}","browser_download_url":"{base}/dl/{win}"}},{{"name":"{unix}","browser_download_url":"{base}/dl/{unix}"}}]}}]"#,
        tag = version_tag,
        win = asset_win,
        unix = asset_unix,
        base = base,
    )
}

// ─── 测试环境（沿用 uninstall.rs 惯例）──────────────────────────

struct MockEnv;

impl EnvOperation for MockEnv {
    fn set_sdk_envs(&self, _envs: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }
    fn add_sdk_path(&self, _sdk_path: &str) -> Result<()> {
        Ok(())
    }
    fn get_path(&self) -> Result<String> {
        Ok(String::new())
    }
    fn remove_sdk_path(&self, _sdk_path: &str) -> Result<()> {
        Ok(())
    }
    fn get_env_value(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn unset_sdk_env(&self, _key: &str) -> Result<()> {
        Ok(())
    }
    fn restore_sdk_envs(&self, _old_envs: &HashMap<String, Option<String>>) -> Result<()> {
        Ok(())
    }
}

struct TestEnv {
    temp: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        unsafe {
            env::remove_var("SDKM_HOME");
        }
        let _ = fs::remove_dir_all(&self.temp);
    }
}

fn setup(config: &SdkmConfig) -> TestEnv {
    let lock = LOCK.lock().unwrap();
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = env::temp_dir().join(format!("sdkm_core_flow_{}_{}", process::id(), id));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(temp.join("store")).unwrap();
    fs::create_dir_all(temp.join("links")).unwrap();
    fs::write(temp.join("config.toml"), toml::to_string(config).unwrap()).unwrap();
    let temp_str = temp.to_str().unwrap().to_string();
    unsafe {
        env::set_var("SDKM_HOME", &temp_str);
    }
    TestEnv { temp, _lock: lock }
}

fn make_manager(config: SdkmConfig) -> SdkManager {
    SdkManager {
        config,
        env_operation: Box::new(MockEnv),
    }
}

/// 最小 zip 字节流：根 README.txt + 可选 bin/demo.txt（多顶层条目避免 normalize 单子目录提升）
fn minimal_zip_at(bin_prefix: bool) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        w.start_file("README.txt", zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut w, b"readme").unwrap();
        let entry = if bin_prefix { "bin/demo.txt" } else { "demo.txt" };
        w.start_file(entry, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut w, b"payload").unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

fn minimal_zip() -> Vec<u8> {
    minimal_zip_at(false)
}

fn assert_installed(env: &TestEnv, sdk: &str, ver: &str, payload_rel: &str) {
    let dir = env.temp.join("store").join(sdk).join(ver);
    assert!(dir.exists(), "version dir missing: {}", dir.display());
    assert!(
        dir.join(payload_rel).exists(),
        "payload missing: {}",
        dir.join(payload_rel).display()
    );
}

/// 断言磁盘 config 的 current_version
fn assert_current(sdk: &str, ver: &str) {
    let conf = SdkmConfig::read_from_disk().unwrap();
    let cur = conf.sdks.iter().find(|s| s.name == sdk).unwrap().current_version.clone();
    assert_eq!(cur.as_deref(), Some(ver));
}

// ─── 用例 ───────────────────────────────────────────────────────

/// 标准 B（GitHub Releases）：GH API URL → 引擎解析 → 平台直链 → root 布局安装
#[test]
fn core_flow_github_releases_direct_link() {
    let zip = minimal_zip();
    // 下载 server 与 releases server 分开起；body 里携带下载 server 的真实地址
    let dl_base = serve_routes(vec![
        ("/dl/tool-x86_64-pc-windows-msvc.zip", zip.clone()),
        ("/dl/tool-x86_64-unknown-linux-gnu.tar.gz", zip),
    ]);
    let body = gh_releases_body(
        &dl_base,
        "tool-x86_64-pc-windows-msvc.zip",
        "tool-x86_64-unknown-linux-gnu.tar.gz",
        "v1.4.2",
    );
    let api_base = serve_routes(vec![("/api/repos/x/tool/releases", body.into_bytes())]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "tool".to_string(),
            version_url: Some(format!("{}/api/repos/x/tool/releases", api_base)),
            version_fallback_url: None,
            download_url: None, // 标准 B：无模板，靠 GH 直链
            download_fallback_url: None,
            current_version: None,
            bin_dir: None, // 根布局
            extra_vars: HashMap::new(),
            extra_paths: Vec::new(),
            os_style: None,
            arch_style: None,
        }],
        symlink_dir: None,
    };
    let env = setup(&config);
    let mut mgr = make_manager(config);

    let sdk = "tool".parse().unwrap();
    mgr.install_sdk(&sdk, "1.4.2", true).unwrap();

    assert_installed(&env, "tool", "1.4.2", "demo.txt");
    assert_current("tool", "1.4.2");
}

/// 标准 A（模板直链）：版本列表 JSON + download_url 模板渲染 → 解压安装
#[test]
fn core_flow_template_download() {
    let zip = minimal_zip();
    let base = serve_routes(vec![
        ("/ver.json", br#"["2.10.0","2.9.0"]"#.to_vec()),
        ("/dist/tool-2.10.0-bin.zip", minimal_zip_at(true)),
    ]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "tmpl".to_string(),
            version_url: Some(format!("{}/ver.json", base)),
            version_fallback_url: None,
            // 标准 A：模板渲染 {version}；写死 .zip 避免 {ext} 跨平台差异干扰断言
            download_url: Some(format!("{}/dist/tool-{{version}}-bin.zip", base)),
            download_fallback_url: None,
            current_version: None,
            bin_dir: Some("bin".to_string()),
            extra_vars: HashMap::new(),
            extra_paths: Vec::new(),
            os_style: None,
            arch_style: None,
        }],
        symlink_dir: None,
    };
    let env = setup(&config);
    let mut mgr = make_manager(config);

    // 模糊匹配：2.10 → 命中列表最新的 2.10.0
    let sdk = "tmpl".parse().unwrap();
    mgr.install_sdk(&sdk, "2.10.0", true).unwrap();

    assert_installed(&env, "tmpl", "2.10.0", &format!("bin{}demo.txt", std::path::MAIN_SEPARATOR));
    assert_current("tmpl", "2.10.0");
}

/// 安装 + switch 符号链接链路（回归保护：引擎型 Custom SDK 走既有 switch 语义）
#[test]
fn switch_symlink_resolves_to_installed_version() {
    let zip = minimal_zip();
    let dl_base = serve_routes(vec![
        ("/dl/tool-x86_64-pc-windows-msvc.zip", zip.clone()),
        ("/dl/tool-x86_64-unknown-linux-gnu.tar.gz", zip),
    ]);
    let body = gh_releases_body(
        &dl_base,
        "tool-x86_64-pc-windows-msvc.zip",
        "tool-x86_64-unknown-linux-gnu.tar.gz",
        "v0.9.9",
    );
    let api_base = serve_routes(vec![("/api/repos/x/tool/releases", body.into_bytes())]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "tool".to_string(),
            version_url: Some(format!("{}/api/repos/x/tool/releases", api_base)),
            version_fallback_url: None,
            download_url: None,
            download_fallback_url: None,
            current_version: None,
            bin_dir: None,
            extra_vars: HashMap::new(),
            extra_paths: Vec::new(),
            os_style: None,
            arch_style: None,
        }],
        symlink_dir: None,
    };
    let env = setup(&config);
    let mut mgr = make_manager(config);

    let sdk = "tool".parse().unwrap();
    mgr.install_sdk(&sdk, "0.9.9", false).unwrap(); // 不自动切换
    mgr.switch_sdk_to_version(&sdk, "0.9.9").unwrap();

    // links/tool → store/tool/0.9.9
    let link = env.temp.join("links").join("tool");
    let target = read_symlink_target(&link).unwrap().expect("symlink should exist after switch");
    let target_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    assert_eq!(target_name, "0.9.9");
}
