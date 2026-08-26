// uninstall 集成测试（参考 rustup 粒度：SDKM_HOME 注入临时 home + mock env + in-process）
//
// 仅覆盖 uninstall 受影响范围。串行执行（SDKM_HOME 是全局环境变量，set_var 需串行避免竞态）。

use anyhow::Result;
use sdkcore::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkcore::env::EnvOperation;
use sdkcore::link::symlink::{create_symlink, read_symlink_target};
use sdkcore::manager::SdkManager;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

// 串行锁：env::set_var("SDKM_HOME") 是全局状态，多线程并行会竞态
static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// mock env 记录的状态：remove_sdk_path / unset_sdk_env 的调用
#[derive(Default)]
struct MockState {
    removed_paths: Vec<String>,
    unset_envs: Vec<String>,
}

struct MockEnv {
    state: Rc<RefCell<MockState>>,
}

impl EnvOperation for MockEnv {
    fn set_sdk_envs(&self, _envs: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }
    fn add_sdk_path(&self, _sdk_path: &str) -> Result<()> {
        Ok(())
    }
    fn get_path(&self) -> Result<String> {
        Ok(String::new()) // 空 PATH → switch 判定 need_add_path=true
    }
    fn remove_sdk_path(&self, sdk_path: &str) -> Result<()> {
        self.state.borrow_mut().removed_paths.push(sdk_path.to_string());
        Ok(())
    }
    fn get_env_value(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn unset_sdk_env(&self, key: &str) -> Result<()> {
        self.state.borrow_mut().unset_envs.push(key.to_string());
        Ok(())
    }
    fn restore_sdk_envs(&self, _old_envs: &HashMap<String, Option<String>>) -> Result<()> {
        Ok(())
    }
}

/// 测试环境：持有串行锁 + 临时 home，Drop 时恢复 SDKM_HOME 并清理临时目录
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

/// 建临时 home 并注入 SDKM_HOME，预建 store/links 目录，写出 config.toml
fn setup(config: &SdkmConfig) -> TestEnv {
    let lock = LOCK.lock().unwrap();
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = env::temp_dir().join(format!("sdkm_uninstall_{}_{}", process::id(), id));
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

/// 构造 java SdkConfig
fn java_sdk(current: Option<&str>, bin_dir: Option<&str>, extra_vars: &[(&str, &str)]) -> SdkConfig {
    let mut vars = HashMap::new();
    for (k, v) in extra_vars {
        vars.insert((*k).to_string(), (*v).to_string());
    }
    SdkConfig {
        name: "java".to_string(),
        version_url: None,
        version_fallback_url: None,
        download_url: Some("http://fake/java".to_string()),
        download_fallback_url: None,
        current_version: current.map(|s| s.to_string()),
        bin_dir: bin_dir.map(|s| s.to_string()),
        extra_vars: vars,
        extra_paths: Vec::new(),
    }
}

/// 构造 SdkManager（内存 config + mock env），返回 manager 与共享 mock 状态
fn make_manager(config: SdkmConfig) -> (SdkManager, Rc<RefCell<MockState>>) {
    let state = Rc::new(RefCell::new(MockState::default()));
    let manager = SdkManager {
        config,
        env_operation: Box::new(MockEnv { state: state.clone() }),
    };
    (manager, state)
}

/// 在 store 下建版本目录
fn make_version_dir(env: &TestEnv, sdk: &str, ver: &str) -> PathBuf {
    let dir = env.temp.join("store").join(sdk).join(ver);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 读取磁盘 config 的 current_version（端到端验证写入生效）
fn disk_current(sdk: &str) -> Option<String> {
    SdkmConfig::read_from_disk()
        .ok()
        .and_then(|c| c.sdks.iter().find(|s| s.name == sdk).and_then(|s| s.current_version.clone()))
}

/// 非 active 版本卸载：仅删目录，current/环境不变
#[test]
fn uninstall_non_active_keeps_current() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_sdk(Some("21"), Some("bin"), &[])],
    };
    let env = setup(&config);
    let java21 = make_version_dir(&env, "java", "21");
    let java17 = make_version_dir(&env, "java", "17");

    let (mut manager, state) = make_manager(config);
    let sdk = manager.match_valid_sdk("java").unwrap();
    manager.uninstall_sdk(&sdk, "17", true).unwrap();

    assert!(!java17.exists(), "java/17 dir should be removed");
    assert!(java21.exists(), "java/21 dir should remain");
    assert_eq!(disk_current("java"), Some("21".to_string()), "current should stay 21");
    let s = state.borrow();
    assert!(s.removed_paths.is_empty(), "non-active uninstall must not touch PATH");
    assert!(s.unset_envs.is_empty(), "non-active uninstall must not touch env");
}

/// active 且有其他版本：先 switch 到其他版本再删，current 指向新版本
#[test]
fn uninstall_active_with_other_switches() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_sdk(Some("21"), Some("bin"), &[])],
    };
    let env = setup(&config);
    let java21 = make_version_dir(&env, "java", "21");
    let java17 = make_version_dir(&env, "java", "17");
    let links_java = env.temp.join("links").join("java");

    let (mut manager, _state) = make_manager(config);
    let sdk = manager.match_valid_sdk("java").unwrap();
    manager.uninstall_sdk(&sdk, "21", true).unwrap();

    assert!(!java21.exists(), "active java/21 dir should be removed");
    assert!(java17.exists(), "java/17 should remain (switched target)");
    assert_eq!(disk_current("java"), Some("17".to_string()), "current should switch to 17");
    let target = read_symlink_target(&links_java).unwrap();
    assert!(target.is_some(), "links/java symlink should exist (pointing to 17)");
}

/// active 且仅此一版：清理 symlink/PATH/env/current 后删目录
#[test]
fn uninstall_active_only_cleans_env() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_sdk(Some("21"), Some("bin"), &[("JAVA_HOME", "{sdk_dir}")])],
    };
    let env = setup(&config);
    let java21 = make_version_dir(&env, "java", "21");
    let links_java = env.temp.join("links").join("java");
    // 预建指向 21 的 symlink（cleanup 应删它）
    create_symlink(&java21, &links_java).unwrap();

    let (mut manager, state) = make_manager(config);
    let sdk = manager.match_valid_sdk("java").unwrap();
    manager.uninstall_sdk(&sdk, "21", true).unwrap();

    assert!(!java21.exists(), "java/21 dir should be removed");
    assert!(!links_java.exists(), "symlink should be cleaned");
    assert_eq!(disk_current("java"), None, "current should be cleared to None");
    assert!(
        !env.temp.join("store").join("java").exists(),
        "empty store/java should be cleaned"
    );
    let s = state.borrow();
    assert!(s.unset_envs.iter().any(|k| k == "JAVA_HOME"), "should unset JAVA_HOME");
    assert!(
        s.removed_paths
            .iter()
            .any(|p| p.ends_with("java") || p.ends_with("java\\bin") || p.ends_with("java/bin")),
        "should remove main bin path, got {:?}",
        s.removed_paths
    );
}

/// 不存在的版本：fuzzy no-match bail，不删任何目录
#[test]
fn uninstall_nonexistent_version_bails() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_sdk(Some("21"), Some("bin"), &[])],
    };
    let env = setup(&config);
    let java21 = make_version_dir(&env, "java", "21");

    let (mut manager, _state) = make_manager(config);
    let sdk = manager.match_valid_sdk("java").unwrap();
    let res = manager.uninstall_sdk(&sdk, "99", true);
    assert!(res.is_err(), "uninstalling nonexistent version should bail");
    assert!(java21.exists(), "java/21 should remain untouched");
}
