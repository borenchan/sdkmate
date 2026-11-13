// `sdkm current` 三层生效版本解析的集成测试（数据层断言 version + origin）。
//
// 覆盖三层优先级、模糊 pin 精确化、失效 pin 降级、子目录向上查找；其中
// stale_session_pin_suppresses_project 是与 shell/env.rs 优先级契约的漂移守卫。

use anyhow::Result;
use sdkm_core::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkm_core::current::VersionOrigin;
use sdkm_core::env::{EnvAction, EnvOperation};
use sdkm_core::manager::SdkManager;
use sdkm_core::report::null_sink;
use sdkm_util::consts::PROJECT_CONFIG_FILE_NAME;
use sdkm_util::sdk::Sdk;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

// 串行锁：SDKM_HOME/set_var/set_current_dir 是全局状态，多线程并行会竞态
static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// mock env：所有方法空实现（查询路径不写环境，env_operation 仅占位满足 SdkManager 字段）
struct MockEnv;

impl EnvOperation for MockEnv {
    fn set_sdk_envs(&self, _: &HashMap<String, String>) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn add_sdk_path(&self, _: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn get_path(&self) -> Result<String> {
        Ok(String::new())
    }
    fn remove_sdk_path(&self, _: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn get_env_value(&self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn unset_sdk_env(&self, _: &str) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
    fn restore_sdk_envs(&self, _: &HashMap<String, Option<String>>) -> Result<Vec<EnvAction>> {
        Ok(Vec::new())
    }
}

/// 测试环境：持有串行锁 + 临时 home + 记录原始 cwd/会话变量，Drop 恢复并清理目录
struct TestEnv {
    temp: PathBuf,
    original_cwd: PathBuf,
    orig_session: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original_cwd);
        unsafe {
            env::remove_var("SDKM_HOME");
            match self.orig_session.clone() {
                Some(v) => env::set_var("SDKM_ACTIVE_JAVA", v),
                None => env::remove_var("SDKM_ACTIVE_JAVA"),
            }
        }
        let _ = fs::remove_dir_all(&self.temp);
    }
}

/// 建临时 home + 注入 SDKM_HOME + 清除 SDKM_ACTIVE_JAVA（防开发机会话变量泄入）
fn setup(config: &SdkmConfig) -> TestEnv {
    let lock = LOCK.lock().unwrap();
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = env::temp_dir().join(format!("sdkm_current_{}_{}", process::id(), id));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(temp.join("store")).unwrap();
    fs::write(temp.join("config.toml"), toml::to_string(config).unwrap()).unwrap();
    let original_cwd = env::current_dir().unwrap();
    let orig_session = env::var("SDKM_ACTIVE_JAVA").ok();
    unsafe {
        env::set_var("SDKM_HOME", temp.to_str().unwrap());
        env::remove_var("SDKM_ACTIVE_JAVA");
    }
    TestEnv {
        temp,
        original_cwd,
        orig_session,
        _lock: lock,
    }
}

/// java switch-only 配置（global 层版本由调用方按需设 current_version）
fn java_config(current: Option<&str>) -> SdkConfig {
    SdkConfig {
        name: "java".to_string(),
        version_url: None,
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        current_version: current.map(str::to_string),
        bin_dir: Some("bin".to_string()),
        extra_vars: HashMap::new(),
        extra_paths: Vec::new(),
        os_style: None,
        arch_style: None,
    }
}

fn make_manager(config: SdkmConfig) -> SdkManager {
    SdkManager {
        config,
        env_operation: Box::new(MockEnv),
        sink: null_sink(),
    }
}

/// 在 store 下建版本目录（目录名 = 版本号，list_local_sdk_versions 据此扫描）
fn make_version_dir(env: &TestEnv, sdk: &str, ver: &str) -> PathBuf {
    let dir = env.temp.join("store").join(sdk).join(ver).join("bin");
    fs::create_dir_all(&dir).unwrap();
    env.temp.join("store").join(sdk).join(ver)
}

/// 建临时项目目录 + 写 `.sdkm.toml` pin + cd 进去（find_project_config 走 current_dir）
fn setup_project(env: &TestEnv, pins: &[(&str, &str)]) -> PathBuf {
    let proj = env.temp.join("proj");
    fs::create_dir_all(&proj).unwrap();
    let content = pins.iter().map(|(k, v)| format!("{} = \"{}\"\n", k, v)).collect::<String>();
    fs::write(proj.join(PROJECT_CONFIG_FILE_NAME), content).unwrap();
    env::set_current_dir(&proj).unwrap();
    proj
}

fn set_session(version: &str) {
    unsafe {
        env::set_var("SDKM_ACTIVE_JAVA", version);
    }
}

// ===================== 三层优先级与来源归属 =====================

/// 三层全设：会话层覆盖项目 pin 与全局 current
#[test]
fn session_overrides_project_and_global() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    make_version_dir(&env, "java", "21.0.2+9");
    make_version_dir(&env, "java", "25.0.0");
    setup_project(&env, &[("java", "21")]);
    set_session("25.0.0");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "25.0.0");
    assert_eq!(ev.origin, VersionOrigin::Session);
}

/// 项目 pin 模糊输入解析为精确版本，来源附配置文件路径
#[test]
fn project_pin_fuzzy_resolves_with_path() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    make_version_dir(&env, "java", "21.0.2+9");
    setup_project(&env, &[("java", "21")]);
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "21.0.2+9");
    // 期望路径取 cwd 本身：macOS getcwd 会解析符号链接（/var → /private/var），
    // core 从 current_dir 向上爬，origin 必然是 cwd 的物理形式
    assert_eq!(
        ev.origin,
        VersionOrigin::Project(env::current_dir().unwrap().join(".sdkm.toml"))
    );
}

/// 无会话无 pin：全局层 current_version 兜底
#[test]
fn global_when_no_pins() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "17.0.10");
    assert_eq!(ev.origin, VersionOrigin::Global);
}

/// 漂移守卫：会话 pin 失效（99 未装）时压制项目 pin 直落全局——镜像 env.rs 项目层跳过规则
#[test]
fn stale_session_pin_suppresses_project() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    make_version_dir(&env, "java", "21.0.2+9");
    setup_project(&env, &[("java", "21")]);
    set_session("99");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "17.0.10");
    assert_eq!(ev.origin, VersionOrigin::Global, "失效会话 pin 应压制项目层直落全局");
}

/// 项目 pin 失效（99 未装）：静默降级全局
#[test]
fn stale_project_pin_falls_back_to_global() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    setup_project(&env, &[("java", "99")]);
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "17.0.10");
    assert_eq!(ev.origin, VersionOrigin::Global);
}

/// 三层全无：返回 None（展示层显示 N/A）
#[test]
fn none_when_nothing_effective() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(None)],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "21.0.2+9");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    assert!(manager.effective_version(&java).is_none());
}

/// 子目录查询：向上查找命中父级 `.sdkm.toml`，来源路径指向父级文件
#[test]
fn project_config_found_from_subdirectory() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config(Some("17.0.10"))],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "17.0.10");
    make_version_dir(&env, "java", "21.0.2+9");
    let proj = setup_project(&env, &[("java", "21")]);
    let subdir = proj.join("sub");
    fs::create_dir_all(&subdir).unwrap();
    env::set_current_dir(&subdir).unwrap();
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    let ev = manager.effective_version(&java).unwrap();
    assert_eq!(ev.version, "21.0.2+9");
    // 期望路径取 cwd 物理形式的父目录：macOS getcwd 解析符号链接（/var → /private/var），
    // core 从 current_dir 向上爬，origin 必然基于 cwd 的物理形式；不可用 canonicalize
    // （Windows 上会加 \\?\ 前缀反而失配）
    let phys_proj = env::current_dir().unwrap().parent().unwrap().to_path_buf();
    assert_eq!(ev.origin, VersionOrigin::Project(phys_proj.join(".sdkm.toml")));
}
