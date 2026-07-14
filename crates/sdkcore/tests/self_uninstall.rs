// self uninstall 集成测试：验证清理所有激活 SDK 环境 + 删除 home 目录内容
//
// 串行执行（SDKM_HOME 全局环境变量，set_var 需串行避免竞态）。
// helper 与 tests/uninstall.rs 各自独立（Rust 集成测试各 binary 独立，不共享私有 helper）。

use anyhow::Result;
use sdkcore::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkcore::env::EnvOperation;
use sdkcore::link::symlink::create_symlink;
use sdkcore::manager::SdkManager;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

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
        Ok(String::new())
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

struct TestEnv {
    temp: PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("SDKM_HOME");
        }
        let _ = std::fs::remove_dir_all(&self.temp);
    }
}

fn setup(config: &SdkmConfig) -> TestEnv {
    let lock = LOCK.lock().unwrap();
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = std::env::temp_dir().join(format!("sdkm_selfun_{}_{}", std::process::id(), id));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join("store")).unwrap();
    std::fs::create_dir_all(temp.join("links")).unwrap();
    std::fs::write(temp.join("config.toml"), toml::to_string(config).unwrap()).unwrap();
    let temp_str = temp.to_str().unwrap().to_string();
    unsafe {
        std::env::set_var("SDKM_HOME", &temp_str);
    }
    TestEnv { temp, _lock: lock }
}

fn make_sdk(name: &str, current: &str, bin_dir: &str, extra_vars: &[(&str, &str)]) -> SdkConfig {
    let mut vars = HashMap::new();
    for (k, v) in extra_vars {
        vars.insert((*k).to_string(), (*v).to_string());
    }
    SdkConfig {
        name: name.to_string(),
        version_url: None,
        version_fallback_url: None,
        download_url: format!("http://fake/{name}"),
        download_fallback_url: None,
        current_version: Some(current.to_string()),
        bin_dir: Some(bin_dir.to_string()),
        extra_vars: vars,
        extra_paths: Vec::new(),
    }
}

fn make_manager(config: SdkmConfig) -> (SdkManager, Rc<RefCell<MockState>>) {
    let state = Rc::new(RefCell::new(MockState::default()));
    let manager = SdkManager {
        config,
        env_operation: Box::new(MockEnv { state: state.clone() }),
    };
    (manager, state)
}

fn make_version_dir(temp: &Path, sdk: &str, ver: &str) -> PathBuf {
    let dir = temp.join("store").join(sdk).join(ver);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 卸载 sdkm 自身：清理所有激活 SDK 环境 + 删除 home 目录内容
#[test]
fn self_uninstall_cleans_all_and_removes_home() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![
            make_sdk("java", "21", "bin", &[("JAVA_HOME", "{sdk_dir}")]),
            make_sdk("maven", "3.9.16", "bin", &[("MAVEN_HOME", "{sdk_dir}")]),
        ],
    };
    let env = setup(&config);
    let java21 = make_version_dir(&env.temp, "java", "21");
    let maven = make_version_dir(&env.temp, "maven", "3.9.16");
    let links_java = env.temp.join("links").join("java");
    let links_maven = env.temp.join("links").join("maven");
    create_symlink(&java21, &links_java).unwrap();
    create_symlink(&maven, &links_maven).unwrap();

    let (mut manager, state) = make_manager(config);
    manager.uninstall_self(true).unwrap();

    // home 内容已删
    assert!(!env.temp.join("store").exists(), "store dir should be removed");
    assert!(!env.temp.join("links").exists(), "links dir should be removed");
    assert!(!env.temp.join("config.toml").exists(), "config.toml should be removed");
    assert!(!java21.exists() && !maven.exists(), "version dirs gone with store");

    // 两个激活 SDK 的环境都清理了
    let s = state.borrow();
    assert!(s.unset_envs.iter().any(|k| k == "JAVA_HOME"), "should unset JAVA_HOME");
    assert!(s.unset_envs.iter().any(|k| k == "MAVEN_HOME"), "should unset MAVEN_HOME");
    assert!(
        s.removed_paths
            .iter()
            .any(|p| p.ends_with("java") || p.ends_with("java\\bin") || p.ends_with("java/bin")),
        "should remove java path, got {:?}",
        s.removed_paths
    );
    assert!(
        s.removed_paths
            .iter()
            .any(|p| p.ends_with("maven") || p.ends_with("maven\\bin") || p.ends_with("maven/bin")),
        "should remove maven path, got {:?}",
        s.removed_paths
    );
}
