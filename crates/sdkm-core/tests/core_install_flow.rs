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
use sdkm_core::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkm_core::env::{EnvAction, EnvOperation};
use sdkm_core::link::symlink::read_symlink_target;
use sdkm_core::manager::SdkManager;
use sdkm_core::report::{CoreEvent, EventSink, ProgressHandle, ProgressKind, null_sink};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process;
use std::sync::{
    Arc, Mutex, MutexGuard,
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

/// 构造最小 GH Releases JSON（一个 stable release：资产表 (文件名，下载路径) 对，覆盖 CI 矩阵全宿主）
fn gh_releases_body(base: &str, version_tag: &str, assets: &[(&str, &str)]) -> String {
    let items: Vec<String> = assets
        .iter()
        .map(|(name, path)| format!(r#"{{"name":"{name}","browser_download_url":"{base}{path}"}}"#))
        .collect();
    format!(
        r#"[{{"tag_name":"{tag}","prerelease":false,"draft":false,"assets":[{list}]}}]"#,
        tag = version_tag,
        list = items.join(",")
    )
}

/// 标准 B mock 资产表：CI 四宿主（windows-x64 / linux-x64 / linux-arm64 / mac-arm64）各一条，
/// 任一 runner 都能命中本平台资产（不命中且无模板时 install 会报错）
const TOOL_ASSETS: &[(&str, &str)] = &[
    ("tool-x86_64-pc-windows-msvc.zip", "/dl/tool-x86_64-pc-windows-msvc.zip"),
    (
        "tool-x86_64-unknown-linux-gnu.tar.gz",
        "/dl/tool-x86_64-unknown-linux-gnu.tar.gz",
    ),
    (
        "tool-aarch64-unknown-linux-gnu.tar.gz",
        "/dl/tool-aarch64-unknown-linux-gnu.tar.gz",
    ),
    ("tool-aarch64-apple-darwin.tar.gz", "/dl/tool-aarch64-apple-darwin.tar.gz"),
];

/// 标准 B mock 的下载路由：windows 资产回 zip，linux/darwin 资产回真实 tar.gz
/// （历史上 linux 路由误回 zip 字节流，linux 宿主解压报 invalid gzip header——Windows 盲区）
fn tool_dl_routes(zip: Vec<u8>) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("/dl/tool-x86_64-pc-windows-msvc.zip", zip),
        ("/dl/tool-x86_64-unknown-linux-gnu.tar.gz", minimal_tar_gz()),
        ("/dl/tool-aarch64-unknown-linux-gnu.tar.gz", minimal_tar_gz()),
        ("/dl/tool-aarch64-apple-darwin.tar.gz", minimal_tar_gz()),
    ]
}

// ─── 测试环境（沿用 uninstall.rs 惯例）──────────────────────────

struct MockEnv;

impl EnvOperation for MockEnv {
    fn set_sdk_envs(&self, _envs: &HashMap<String, String>) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn add_sdk_path(&self, _sdk_path: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn get_path(&self) -> Result<String> {
        Ok(String::new())
    }
    fn remove_sdk_path(&self, _sdk_path: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn get_env_value(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn unset_sdk_env(&self, _key: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn restore_sdk_envs(&self, _old_envs: &HashMap<String, Option<String>>) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
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
        sink: null_sink(),
    }
}

// ─── MockSink：统一记录事件 + 进度生命周期（GUI sink 的测试替身）────────

/// 事件与进度共用一条日志流（可按序断言交错关系）：
/// 事件 push `Debug` 格式原文；进度 push `progress <kind>: <title>` 与 `pb <方法>=<值>`
struct MockSink(Arc<Mutex<Vec<String>>>);

impl EventSink for MockSink {
    fn emit(&self, ev: CoreEvent) {
        self.0.lock().unwrap().push(format!("{ev:?}"));
    }
    fn progress(&self, kind: ProgressKind, title: String) -> Box<dyn ProgressHandle> {
        self.0.lock().unwrap().push(format!("progress {kind:?}: {title}"));
        Box::new(MockProgress(self.0.clone()))
    }
}

/// MockSink 配套进度句柄：生命周期调用全量入流
struct MockProgress(Arc<Mutex<Vec<String>>>);

impl ProgressHandle for MockProgress {
    fn set_length(&self, total: u64) {
        self.0.lock().unwrap().push(format!("pb set_length={total}"));
    }
    fn set_position(&self, pos: u64) {
        self.0.lock().unwrap().push(format!("pb set_position={pos}"));
    }
    fn inc(&self, delta: u64) {
        self.0.lock().unwrap().push(format!("pb inc={delta}"));
    }
    fn set_message(&self, msg: String) {
        self.0.lock().unwrap().push(format!("pb set_message={msg}"));
    }
    fn finish_with_message(&self, msg: String) {
        self.0.lock().unwrap().push(format!("pb finish={msg}"));
    }
    fn finish_and_clear(&self) {
        self.0.lock().unwrap().push("pb finish_and_clear".to_string());
    }
}

fn make_manager_recording(config: SdkmConfig) -> (SdkManager, Arc<Mutex<Vec<String>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    (
        SdkManager {
            config,
            env_operation: Box::new(MockEnv),
            sink: Arc::new(MockSink(events.clone())),
        },
        events,
    )
}

/// 断言各 pattern 依序出现（允许中间夹其他记录；路径等动态内容用子串匹配）
fn assert_in_order(log: &[String], patterns: &[&str]) {
    let mut cursor = 0;
    for p in patterns {
        let found = log[cursor..]
            .iter()
            .position(|m| m.contains(p))
            .unwrap_or_else(|| panic!("pattern `{p}` not found in order, log: {log:?}"));
        cursor += found + 1;
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

/// 最小 tar.gz 字节流（根布局 README.txt + demo.txt）：linux/darwin 资产的真实载荷
fn minimal_tar_gz() -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    let mut add = |name: &str, data: &[u8]| {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, data)
    };
    add("README.txt", b"readme").unwrap();
    add("demo.txt", b"payload").unwrap();
    tar.into_inner().unwrap().finish().unwrap()
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
    // 下载 server 与 releases server 分开起；body 里携带下载 server 的真实地址
    let dl_base = serve_routes(tool_dl_routes(minimal_zip()));
    let body = gh_releases_body(&dl_base, "v1.4.2", TOOL_ASSETS);
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
    let resolved = mgr.resolve_install_version(&sdk, "1.4.2").unwrap();
    mgr.install_sdk(&sdk, &resolved, true).unwrap();

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
    let resolved = mgr.resolve_install_version(&sdk, "2.10.0").unwrap();
    mgr.install_sdk(&sdk, &resolved, true).unwrap();

    assert_installed(&env, "tmpl", "2.10.0", &format!("bin{}demo.txt", std::path::MAIN_SEPARATOR));
    assert_current("tmpl", "2.10.0");
}

/// install 全流程事件 + 进度生命周期：core 零打印，文案/进度逐条经 sink 可断言
#[test]
fn core_flow_emits_event_sequence() {
    let base = serve_routes(vec![
        ("/ver.json", br#"["1.4.2"]"#.to_vec()),
        ("/dist/tool-1.4.2-bin.zip", minimal_zip_at(true)),
    ]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "ev".to_string(),
            version_url: Some(format!("{}/ver.json", base)),
            version_fallback_url: None,
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
    let (mut mgr, events) = make_manager_recording(config);

    let sdk = "ev".parse().unwrap();
    let resolved = mgr.resolve_install_version(&sdk, "1.4.2").unwrap();
    mgr.install_sdk(&sdk, &resolved, true).unwrap();

    assert_installed(&env, "ev", "1.4.2", &format!("bin{}demo.txt", std::path::MAIN_SEPARATOR));
    let log = events.lock().unwrap().clone();
    // 过程事件：文案按序（进度条 finish 文案走 pb finish= 记录，不混入事件）
    assert_in_order(
        &log,
        &[
            "ev 1.4.2 is not installed locally, proceeding...",
            "Download URL (primary): ",
            "Temporary directory: ",
            "Temporary files cleaned up.",
            "🔗 Switching ev to 1.4.2...",
            "ev 1.4.2 installed and switched successfully!",
        ],
    );
    // 进度生命周期：每场景 创建 → 数据推进(set_length/inc) → 结束(finish)
    assert_in_order(
        &log,
        &[
            "progress Resolve: Resolving version '1.4.2' for ev...",
            "pb finish=✅ Resolved: 1.4.2 → 1.4.2",
            "progress Download: Downloading ev 1.4.2...",
            "pb set_length=", // 服务器 Content-Length
            "pb inc=",        // 逐块推进
            "pb finish=✅ Downloaded ev 1.4.2",
            "progress ExtractZip: Extracting ev 1.4.2...",
            "pb set_length=", // zip 文件总数
            "pb inc=",
            "pb finish=✅ Extracted ev 1.4.2",
            "progress Verify: Verifying installation...",
            "pb finish=📂 Moved to ",
            "progress Verify: Verifying installation...",
            "pb finish=✅ Verified ev 1.4.2",
        ],
    );
}

/// 已装版本路径：Warning + 重装提示事件，不再下载
#[test]
fn core_flow_emits_already_installed() {
    let base = serve_routes(vec![
        ("/ver.json", br#"["1.4.2"]"#.to_vec()),
        ("/dist/tool-1.4.2-bin.zip", minimal_zip_at(true)),
    ]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "ev2".to_string(),
            version_url: Some(format!("{}/ver.json", base)),
            version_fallback_url: None,
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
    // 第一次静默装好，第二次带录制 sink 重装同一版本
    let (mut mgr, events) = make_manager_recording(config.clone());
    let sdk = "ev2".parse().unwrap();
    let resolved = mgr.resolve_install_version(&sdk, "1.4.2").unwrap();
    mgr.install_sdk(&sdk, &resolved, true).unwrap();
    events.lock().unwrap().clear();

    let resolved = mgr.resolve_install_version(&sdk, "1.4.2").unwrap();
    mgr.install_sdk(&sdk, &resolved, false).unwrap();

    let log = events.lock().unwrap().clone();
    assert_in_order(
        &log,
        &[
            "ev2 1.4.2 is already installed locally.",
            "If you want to reinstall, remove it first: delete ",
        ],
    );
    assert!(
        !log.iter().any(|m| m.contains("installed successfully")),
        "已装路径不应再触发安装成功事件: {log:?}"
    );
    assert!(
        !log.iter()
            .any(|m| m.starts_with("progress Download:") || m.starts_with("progress Extract")),
        "已装路径不应再创建下载/解压进度（Resolve 先于已装检查，允许存在）: {log:?}"
    );
    assert_installed(&env, "ev2", "1.4.2", &format!("bin{}demo.txt", std::path::MAIN_SEPARATOR));
}

/// 主源失败→备源切换：主源 bar 收尾（finish_and_clear）、备源 bar 接管并打成功消息，
/// 不再出现"死而未僵的主源 bar 与备源 bar 双活重绘"（终端闪烁根源）
#[test]
fn core_flow_fallback_download_bar_lifecycle() {
    let zip = minimal_zip_at(true);
    let base = serve_routes(vec![
        ("/ver.json", br#"["3.0.0"]"#.to_vec()),
        // 主源路径故意不注册：serve_routes 未匹配返回 404
        ("/fb/tool-3.0.0.zip", zip),
    ]);

    let config = SdkmConfig {
        network: NetworkConfig::default(),
        sdks: vec![SdkConfig {
            name: "fb".to_string(),
            version_url: Some(format!("{}/ver.json", base)),
            version_fallback_url: None,
            download_url: Some(format!("{}/dist/tool-{{version}}-bin.zip", base)),
            download_fallback_url: Some(format!("{}/fb/tool-{{version}}.zip", base)),
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
    let (mut mgr, events) = make_manager_recording(config);

    let sdk = "fb".parse().unwrap();
    let resolved = mgr.resolve_install_version(&sdk, "3.0.0").unwrap();
    mgr.install_sdk(&sdk, &resolved, false).unwrap();

    assert_installed(&env, "fb", "3.0.0", &format!("bin{}demo.txt", std::path::MAIN_SEPARATOR));
    let log = events.lock().unwrap().clone();
    // 事件顺序：主源重试警告 → 切备源警告 → 安装成功（无 Error 事件，最终失败才走 Err）
    assert_in_order(
        &log,
        &[
            "Download attempt #1 failed: ",
            "Download attempt #2 failed: ",
            "Primary download failed, trying fallback URL: ",
            "fb 3.0.0 installed successfully!",
        ],
    );
    // bar 生命周期：主源 bar 创建→被 finish_and_clear 收尾，备源 bar 创建→打 ✅ Downloaded；
    // 主源 bar 不再"复活"打成功消息（此前死而未僵的主源 bar 会突然画一帧 ✅）
    let main_clear_idx = log
        .iter()
        .position(|m| m == "pb finish_and_clear")
        .expect("主源 bar 应被显式收尾");
    let downloads: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, m)| m.starts_with("progress Download: "))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(downloads.len(), 2, "应恰好两条下载 bar（主源+备源）: {log:?}");
    assert!(
        downloads[0] < main_clear_idx && main_clear_idx < downloads[1],
        "顺序应为 主源bar创建 → finish_and_clear → 备源bar创建: {log:?}"
    );
    // ✅ Downloaded 必须出现在备源 bar 之后
    assert_in_order(
        &log,
        &["progress Download: Downloading fb 3.0.0...", "pb finish=✅ Downloaded fb 3.0.0"],
    );
}

/// 安装 + switch 符号链接链路（回归保护：引擎型 Custom SDK 走既有 switch 语义）
#[test]
fn switch_symlink_resolves_to_installed_version() {
    let dl_base = serve_routes(tool_dl_routes(minimal_zip()));
    let body = gh_releases_body(&dl_base, "v0.9.9", TOOL_ASSETS);
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
    let resolved = mgr.resolve_install_version(&sdk, "0.9.9").unwrap();
    mgr.install_sdk(&sdk, &resolved, false).unwrap(); // 不自动切换
    mgr.switch_sdk_to_version(&sdk, "0.9.9").unwrap();

    // links/tool → store/tool/0.9.9
    let link = env.temp.join("links").join("tool");
    let target = read_symlink_target(&link).unwrap().expect("symlink should exist after switch");
    let target_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    assert_eq!(target_name, "0.9.9");
}
