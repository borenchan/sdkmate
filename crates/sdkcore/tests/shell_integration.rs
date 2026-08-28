// shell 后端重构核心入口的集成测试（端到端，参考 rustup 粒度）。
//
// 覆盖三个核心入口（均跨平台，cfg(unix) 路径在 Windows 也能编译——测试只走公共入口，
// 不直接引用 unix-only 函数）：
// - `use_session_version`：会话层 `sdkm use --shell` 脚本生成（四 shell）
// - `generate_env_script_cached`：`sdkm env` 三层解析 + 缓存（最核心高频路径）
// - `HookCache`：跨 shell 串扰防护 + schema 失效
//
// 沙箱：SDKM_HOME 注入临时 home + mock env（generate_env_script 不调 env_operation，
// 占位即可）+ 串行锁（SDKM_HOME/set_current_dir 全局状态，并发竞态）。

use anyhow::Result;
use sdkcore::config::{NetworkConfig, SdkConfig, SdkmConfig};
use sdkcore::env::EnvOperation;
use sdkcore::hook_cache::{CACHE_SCHEMA_VERSION, HookCache, HookEntry, current_session_fingerprint};
use sdkcore::manager::SdkManager;
use sdkcore::project_config::ProjectConfig;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};
use util::consts::PROJECT_CONFIG_FILE_NAME;
use util::sdk::Sdk;
use util::shell::Shell;

// 串行锁：SDKM_HOME 与 set_current_dir 是全局状态，多线程并行会竞态
static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// mock env：所有方法空实现（被测入口不写环境，env_operation 仅占位满足 SdkManager 字段）
struct MockEnv;

impl EnvOperation for MockEnv {
    fn set_sdk_envs(&self, _: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }
    fn add_sdk_path(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn get_path(&self) -> Result<String> {
        Ok(String::new())
    }
    fn remove_sdk_path(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn get_env_value(&self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn unset_sdk_env(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn restore_sdk_envs(&self, _: &HashMap<String, Option<String>>) -> Result<()> {
        Ok(())
    }
}

/// 测试环境：持有串行锁 + 临时 home + 记录原始 cwd，Drop 恢复 SDKM_HOME/cwd/清理目录
struct TestEnv {
    temp: PathBuf,
    original_cwd: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original_cwd);
        unsafe {
            env::remove_var("SDKM_HOME");
        }
        let _ = fs::remove_dir_all(&self.temp);
    }
}

/// 建临时 home + 注入 SDKM_HOME + 预建 store/links + 写 config.toml，返回 TestEnv
fn setup(config: &SdkmConfig) -> TestEnv {
    let lock = LOCK.lock().unwrap();
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = env::temp_dir().join(format!("sdkm_shell_{}_{}", process::id(), id));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(temp.join("store")).unwrap();
    fs::create_dir_all(temp.join("links")).unwrap();
    fs::create_dir_all(temp.join(".cache")).unwrap();
    fs::write(temp.join("config.toml"), toml::to_string(config).unwrap()).unwrap();
    let original_cwd = env::current_dir().unwrap();
    unsafe {
        env::set_var("SDKM_HOME", temp.to_str().unwrap());
    }
    TestEnv {
        temp,
        original_cwd,
        _lock: lock,
    }
}

/// java switch-only 配置（无 version_url/download_url = 本地 switch-only；extra_vars 让 env 有 export 内容）
fn java_config() -> SdkConfig {
    let mut vars = HashMap::new();
    vars.insert("JAVA_HOME".to_string(), "{sdk_dir}".to_string());
    SdkConfig {
        name: "java".to_string(),
        version_url: None,
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        current_version: None,
        bin_dir: Some("bin".to_string()),
        extra_vars: vars,
        extra_paths: Vec::new(),
    }
}

fn make_manager(config: SdkmConfig) -> SdkManager {
    SdkManager {
        config,
        env_operation: Box::new(MockEnv),
    }
}

/// 在 store 下建版本目录（目录名 = 版本号，list_local_sdk_versions 据此扫描）
fn make_version_dir(env: &TestEnv, sdk: &str, ver: &str) -> PathBuf {
    let dir = env.temp.join("store").join(sdk).join(ver).join("bin");
    fs::create_dir_all(&dir).unwrap();
    env.temp.join("store").join(sdk).join(ver)
}

// ===================== use_session_version（会话层 `sdkm use --shell`）=====================

/// 四 shell 各输出正确的 `SDKM_ACTIVE_JAVA` 赋值行（含 shell 语法前缀 + 解析后的精确版本）
#[test]
fn use_session_version_four_shells() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "21.0.2+9");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();

    for shell in Shell::ALL {
        let script = manager.use_session_version(shell, &java, "21").unwrap();
        // 变量名 + 解析后的精确版本（模糊 "21" → "21.0.2+9"）必须出现
        assert!(script.contains("SDKM_ACTIVE_JAVA"), "{shell:?} 缺变量名");
        assert!(script.contains("21.0.2+9"), "{shell:?} 缺精确版本");
        // 各 shell 语法前缀
        match shell {
            Shell::Bash | Shell::Zsh => assert!(script.contains("export SDKM_ACTIVE_JAVA"), "{shell:?} 缺 export"),
            Shell::Fish => assert!(script.contains("set -gx SDKM_ACTIVE_JAVA"), "{shell:?} 缺 set -gx"),
            Shell::PowerShell => assert!(script.contains("$env:SDKM_ACTIVE_JAVA"), "{shell:?} 缺 $env:"),
        }
    }
}

/// 未装版本：use_session_version 必须 Err（会话覆盖要求本地已装可解析）
#[test]
fn use_session_version_uninstalled_bails() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "21.0.2+9");
    let manager = make_manager(config);
    let java = Sdk::from_str("java").unwrap();
    let res = manager.use_session_version(Shell::Bash, &java, "99");
    assert!(res.is_err(), "未装版本应 bail，不允许写入会话覆盖");
}

// ===================== generate_env_script_cached（`sdkm env` 三层解析 + 缓存）=====================

/// 有项目 pin：输出含 java bin 的 PATH 重建行 + export JAVA_HOME
#[test]
fn env_script_with_project_pin() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    let java_dir = make_version_dir(&env, "java", "21.0.2+9");
    // 项目目录 + .sdkm.toml pin java=21（模糊值，验三层解析 + fuzzy 命中）
    let proj = env.temp.join("proj");
    fs::create_dir_all(&proj).unwrap();
    let pins = BTreeMap::from([("java".to_string(), "21".to_string())]);
    let cfg = ProjectConfig { pins };
    fs::write(proj.join(PROJECT_CONFIG_FILE_NAME), toml::to_string_pretty(&cfg).unwrap()).unwrap();
    env::set_current_dir(&proj).unwrap();

    let manager = make_manager(config);
    let script = manager.generate_env_script_cached(Shell::Bash, &proj);

    // PATH 重建行含 java bin 路径（绕过全局 symlink 的 store 真实版本目录）
    let expected_bin = java_dir.join("bin").to_string_lossy().replace('/', "\\");
    let bin_unix: String = java_dir.join("bin").to_string_lossy().into_owned();
    assert!(
        script.contains(&expected_bin) || script.contains(bin_unix.as_str()),
        "PATH 行应含 java bin 路径，got: {script}"
    );
    assert!(script.contains("export JAVA_HOME"), "应 export JAVA_HOME（extra_vars 渲染）");
    assert!(script.contains("21.0.2+9") || script.contains("21"), "JAVA_HOME 值含版本目录");
    // 离开项目 = 无选中 → unset JAVA_HOME（known 幂等还原）；此例有选中不应 unset JAVA_HOME
    assert!(!script.contains("unset JAVA_HOME"), "有选中时不应 unset JAVA_HOME");
}

/// 无项目配置：仅 base 自愈 + PATH(base) + unset JAVA_HOME（known 集合幂等还原），无 export
#[test]
fn env_script_no_project_config() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    // 无 .sdkm.toml 的空项目目录
    let proj = env.temp.join("empty_proj");
    fs::create_dir_all(&proj).unwrap();
    env::set_current_dir(&proj).unwrap();

    let manager = make_manager(config);
    let script = manager.generate_env_script_cached(Shell::Bash, &proj);

    // 无选中 → 不应 export JAVA_HOME；known 集合含 JAVA_HOME → 应 unset
    assert!(!script.contains("export JAVA_HOME="), "无选中不应 export JAVA_HOME");
    assert!(script.contains("unset JAVA_HOME"), "应 unset JAVA_HOME（幂等还原）");
}

/// 同 pwd+shell 两次调用输出一致（缓存命中，零磁盘解析）
#[test]
fn env_script_cached_idempotent() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    make_version_dir(&env, "java", "21.0.2+9");
    let proj = env.temp.join("proj");
    fs::create_dir_all(&proj).unwrap();
    let pins = BTreeMap::from([("java".to_string(), "21".to_string())]);
    fs::write(
        proj.join(PROJECT_CONFIG_FILE_NAME),
        toml::to_string_pretty(&ProjectConfig { pins }).unwrap(),
    )
    .unwrap();
    env::set_current_dir(&proj).unwrap();

    let manager = make_manager(config);
    let first = manager.generate_env_script_cached(Shell::Fish, &proj);
    let second = manager.generate_env_script_cached(Shell::Fish, &proj);
    assert_eq!(first, second, "缓存命中应输出一致脚本");
}

/// 会话变量中途设置：缓存条目必须失效重算（会话 > 项目）——回归用户 bug：
/// 先 `sdkm use`（项目 pin 入缓存）→ `use --shell`（设 SDKM_ACTIVE_*）→
/// hook 再调 env 时缓存命中吐旧脚本，项目版本压过会话版本。
#[test]
fn env_script_recomputes_when_session_var_set() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    let v21_dir = make_version_dir(&env, "java", "21.0.2+9");
    let v25_dir = make_version_dir(&env, "java", "25.0.0");
    let proj = env.temp.join("proj");
    fs::create_dir_all(&proj).unwrap();
    let pins = BTreeMap::from([("java".to_string(), "21".to_string())]);
    fs::write(
        proj.join(PROJECT_CONFIG_FILE_NAME),
        toml::to_string_pretty(&ProjectConfig { pins }).unwrap(),
    )
    .unwrap();
    env::set_current_dir(&proj).unwrap();

    // 防御：清掉测试环境可能残留的真实 SDKM_ACTIVE_JAVA（同 shell 跑过 use --shell 的场景）
    unsafe {
        env::remove_var("SDKM_ACTIVE_JAVA");
    }

    let manager = make_manager(config);
    let before = manager.generate_env_script_cached(Shell::Bash, &proj);
    assert!(
        before.contains(&v21_dir.join("bin").to_string_lossy().replace('/', "\\"))
            || before.contains(&v21_dir.join("bin").to_string_lossy().into_owned()),
        "无会话变量时应选项目 pin（21），got: {before}"
    );

    // 中途设会话变量（模拟 `sdkm use --shell java 25`）——Drop 守卫恢复，防泄给后续测试
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                env::remove_var("SDKM_ACTIVE_JAVA");
            }
        }
    }
    let _guard = Guard;
    unsafe {
        env::set_var("SDKM_ACTIVE_JAVA", "25.0.0");
    }

    let after = manager.generate_env_script_cached(Shell::Bash, &proj);
    let v25_bin_win = after.contains(&v25_dir.join("bin").to_string_lossy().replace('/', "\\"))
        || after.contains(&v25_dir.join("bin").to_string_lossy().into_owned());
    let v21_bin_win = after.contains(&v21_dir.join("bin").to_string_lossy().replace('/', "\\"))
        || after.contains(&v21_dir.join("bin").to_string_lossy().into_owned());
    assert!(v25_bin_win, "会话变量已设，应重算并选会话版本（25），got: {after}");
    assert!(!v21_bin_win, "会话 > 项目，项目 pin（21）不应出现在脚本中");
}

// ===================== HookCache（跨 shell 串扰 + schema 失效）=====================

/// 同 PWD 不同 shell：put(bash) 后 resolve(bash) 命中、resolve(fish) miss（防串扰）
#[test]
fn hook_cache_cross_shell_miss() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    let pwd = env.temp.join("proj");
    fs::create_dir_all(&pwd).unwrap();

    let mut cache = HookCache::load();
    cache.put(
        &pwd,
        HookEntry {
            config_path: String::new(), // 空串 = 无项目配置锚定，resolve 不查 mtime
            mtime_nanos: 0,
            env_script: "echo bash".to_string(),
            schema_version: CACHE_SCHEMA_VERSION,
            shell: Shell::Bash as u8,
            // 跟随当前进程会话指纹，模拟「与 resolve 时同状态」的正常命中
            session_fingerprint: current_session_fingerprint(),
        },
    );

    assert!(cache.resolve(&pwd, Shell::Bash as u8).is_some(), "同 shell 应命中");
    assert!(cache.resolve(&pwd, Shell::Fish as u8).is_none(), "跨 shell 应 miss（防串扰）");
    assert!(cache.resolve(&pwd, Shell::Zsh as u8).is_none(), "跨 shell 应 miss");
    assert!(cache.resolve(&pwd, Shell::PowerShell as u8).is_none(), "跨 shell 应 miss");
}

/// 旧 schema 条目：resolve 当 miss（强制重建，防模板变更后吐旧脚本）
#[test]
fn hook_cache_schema_mismatch_miss() {
    let config = SdkmConfig {
        symlink_dir: None,
        network: NetworkConfig::default(),
        sdks: vec![java_config()],
    };
    let env = setup(&config);
    let pwd = env.temp.join("proj");
    fs::create_dir_all(&pwd).unwrap();

    let mut cache = HookCache::load();
    cache.put(
        &pwd,
        HookEntry {
            config_path: String::new(),
            mtime_nanos: 0,
            env_script: "stale".to_string(),
            schema_version: CACHE_SCHEMA_VERSION - 1, // 旧 schema
            shell: Shell::Bash as u8,
            session_fingerprint: current_session_fingerprint(),
        },
    );

    assert!(
        cache.resolve(&pwd, Shell::Bash as u8).is_none(),
        "旧 schema 应 miss，即使 shell 相符"
    );
}
