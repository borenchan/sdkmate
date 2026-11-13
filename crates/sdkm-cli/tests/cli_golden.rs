//! CLI 集成测试（重构期金样工具的精简永久版）：Command 跑 sdkm 二进制 + SDKM_HOME 沙箱。
//!
//! 断言口径（与临时金样工具的全量字节对比互补，此处做长期回归网）：
//! - exit code 精确；stderr 单行错误文案去 ANSI 后全等（错误路径字节级锁定）
//! - 展示性输出（表格/概览）剥色码后按关键 token 断言——列宽对齐/图标颜色属样式，
//!   全量字节锁定由临时金样工具负责（D:\Desktop\sdkm资料\golden\golden.ps1）
//! - 含宿主 PATH 的脚本（env/use --shell）只断言结构特征 + stdout 纯净性（无 UI 前缀）
//! - 路径归一化为 {FX}；版本号场景前缀匹配（发版演化）

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::process::{Command, Output};

// ── 沙箱 fixture ────────────────────────────────────────────────

/// 每测试独立 SDKM_HOME：config + store 目录，Drop 清理
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    /// java 21.0.5(激活)/8.0.492 + maven 3.9.6 两 SDK 的标准 fixture
    fn new(tag: &str) -> Self {
        let root = env::temp_dir().join(format!("sdkm_golden_{}_{}", process::id(), tag));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("store/java/21.0.5")).unwrap();
        fs::create_dir_all(root.join("store/java/8.0.492")).unwrap();
        fs::create_dir_all(root.join("store/maven/3.9.6")).unwrap();
        fs::create_dir_all(root.join("work")).unwrap();
        fs::write(
            root.join("config.toml"),
            "[network]\nssl_verify = true\nconnect_timeout = 30\ncache_ttl_secs = 3600\n\n\
             [[sdk]]\nname = \"java\"\nversion_url = \"\"\ndownload_url = \"\"\n\
             current_version = \"21.0.5\"\n\
             extra_vars = { JAVA_HOME = \"{sdk_dir}\" }\nextra_paths = []\n",
        )
        .unwrap();
        // 环境污染屏障：空 pins 的项目配置截断 find_project_config 的向上爬取，
        // 防宿主 temp 目录残留的 .sdkm.toml 混入测试三层解析（历史踩坑）
        fs::write(root.join("work/.sdkm.toml"), "# sdkm test isolation barrier\n").unwrap();
        Sandbox { root }
    }

    fn work(&self) -> PathBuf {
        self.root.join("work")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ── 运行与归一化 helper ─────────────────────────────────────────

/// 跑 sdkm（cwd 在 fixture work 目录，额外 env 可注入如 SDKM_ACTIVE_*）
fn run_in(fx: &Sandbox, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sdkm"));
    cmd.args(args)
        .env("SDKM_HOME", &fx.root)
        .current_dir(fx.work())
        .env_remove("SDKM_ACTIVE_JAVA"); // 隔离宿主残留的会话层变量
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().expect("failed to spawn sdkm")
}

/// 输出文本归一化：fixture 路径（原生/正斜杠/反斜杠三形态）→ {FX}，统一 LF
fn norm(fx: &Sandbox, raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw).to_string();
    let native = fx.root.to_string_lossy().to_string();
    let fwd = native.replace('\\', "/");
    let back = native.replace('/', "\\");
    s.replace(&native, "{FX}").replace(&fwd, "{FX}").replace(&back, "{FX}")
}

/// 剥 ANSI SGR 色码序列（\x1b[...m），保留文本内容
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // 跳过到 SGR 终结符 m（与 sdkm_util::terminal::display_width 同款状态机）
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn plain(fx: &Sandbox, raw: &[u8]) -> String {
    let mut s = strip_ansi(&norm(fx, raw));
    // debug 构建的错误路径附加 backtrace 块（release 无），截断后统一断言口径
    if let Some(idx) = s.find("debug log detail:") {
        // head = debug 块自身 "🦀" 起点；截断后串尾保留错误行自己的换行
        let head = idx.saturating_sub("🦀 ".len());
        s.truncate(head);
    }
    s
}

// ── 场景 ────────────────────────────────────────────────────────

/// --version：版本号演化，前缀匹配
#[test]
fn version_output() {
    let fx = Sandbox::new("ver");
    let out = run_in(&fx, &["--version"], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        plain(&fx, &out.stdout).starts_with("sdkm 0."),
        "got: {:?}",
        plain(&fx, &out.stdout)
    );
}

/// current 单 SDK：全局来源标注（去色码后全等——单行无对齐）
#[test]
fn current_global_source() {
    let fx = Sandbox::new("cur_g");
    let out = run_in(&fx, &["current", "java"], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(plain(&fx, &out.stdout), "ℹ️  java 21.0.5 (global)\n");
}

/// current 三层优先级：project pin 与 session 覆盖全局
#[test]
fn current_project_and_session_sources() {
    let fx = Sandbox::new("cur_ps");
    // project：cwd 放 .sdkm.toml
    fs::write(fx.work().join(".sdkm.toml"), "java = \"21\"\n").unwrap();
    let out = run_in(&fx, &["current", "java"], &[]);
    let text = plain(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        text.starts_with("ℹ️  java 21.0.5 (project") && text.contains(".sdkm.toml"),
        "got: {text:?}"
    );

    // session：SDKM_ACTIVE_JAVA 覆盖 project（最高层）
    let out = run_in(&fx, &["current", "java"], &[("SDKM_ACTIVE_JAVA", "8")]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(plain(&fx, &out.stdout), "ℹ️  java 8.0.492 (session)\n");
}

/// current 概览：未激活 SDK 显示 N/A（表格对齐空格不锁，token 断言）
#[test]
fn current_overview_shows_na() {
    let fx = Sandbox::new("cur_na");
    let out = run_in(&fx, &["current"], &[]);
    let text = plain(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(text.contains("java") && text.contains("21.0.5") && text.contains("global"));
    assert!(text.contains("maven") && text.contains("N/A"));
}

/// ls 概览 + 单 SDK 版本列表：active ✅ 标记
#[test]
fn ls_overview_and_version_list() {
    let fx = Sandbox::new("ls");
    let out = run_in(&fx, &["ls"], &[]);
    let text = plain(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(text.contains("java") && text.contains("21.0.5"));
    assert!(text.contains("maven") && text.contains("N/A"));

    let out = run_in(&fx, &["ls", "java"], &[]);
    let text = plain(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    // 顺序无关断言：版本列表来自 read_dir 枚举、未显式排序，NTFS/ext4 枚举序不同
    // （已知既有缺陷，Windows 名序 21 在前、ext4 哈希序可能 8 在前——排序语义待定，未修）
    assert!(text.contains("✅") && text.contains("21.0.5"), "active 标记缺失: {text:?}");
    assert!(text.contains("8.0.492"));
}

/// ls 空表（有 config 无 store）与 config 缺失错误（金样 ls-empty 同款路径）
#[test]
fn ls_empty_and_missing_config() {
    // 有 config、store 空 → 引导安装提示
    let fx = Sandbox::new("ls_empty");
    let _ = fs::remove_dir_all(fx.root.join("store"));
    fs::create_dir_all(fx.root.join("store")).unwrap();
    let out = run_in(&fx, &["ls"], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        plain(&fx, &out.stdout),
        "ℹ️  no installed sdks. run `sdkm install <sdk> <version>` to get started.\n"
    );

    // 无 config → 错误走 stderr，exit 1
    let fx = Sandbox::new("ls_nocfg");
    fs::remove_file(fx.root.join("config.toml")).unwrap();
    let out = run_in(&fx, &["ls"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        plain(&fx, &out.stderr),
        "🦀 Failed to read sdkm config! please try again after executing `sdkm init` in sdkm home dir\n"
    );
    assert!(out.stdout.is_empty(), "错误路径 stdout 应为空");
}

/// config get / list
#[test]
fn config_get_and_list() {
    let fx = Sandbox::new("cfg");
    let out = run_in(&fx, &["config", "get", "network.connect_timeout"], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(plain(&fx, &out.stdout), "ℹ️  network.connect_timeout = 30\n");

    let out = run_in(&fx, &["config", "list"], &[]);
    let text = plain(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(text.contains("symlink_dir = {FX}"), "got: {text:?}");
    assert!(text.contains("network.proxy = (none)"));
    assert!(text.contains("network.connect_timeout = 30"));
}

/// 未注册 SDK：exit 1 + stderr 错误文案（字节级）+ stdout 空（分流契约）
#[test]
fn err_unregistered_switch() {
    let fx = Sandbox::new("err");
    let out = run_in(&fx, &["switch", "foo", "1"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        plain(&fx, &out.stderr),
        "🦀 Unregistered SDK `foo`. Register it with: `sdkm config add-sdk foo` \
         (add `--download-url <url>` for remote install, or omit for a local switch-only SDK).\n"
    );
    assert!(out.stdout.is_empty());
}

/// hook 脚本：纯文本稳定（无色码无宿主路径），全文锁定
#[test]
fn hook_script_stable() {
    let fx = Sandbox::new("hook");
    let out = run_in(&fx, &["hook", "bash"], &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = norm(&fx, &out.stdout);
    assert!(text.starts_with("# sdkm hook -- bash\n"), "got: {text:?}");
    assert!(text.contains("eval \"$(sdkm env --shell bash)\""));
    assert!(text.contains("_SDKM_BASE_PATH"));
    assert!(out.stderr.is_empty());
}

/// env / use --shell 输出纯净性：stdout 只吐脚本（会被 eval），UI 前缀泄漏即失败
#[test]
fn env_and_use_shell_purity() {
    let fx = Sandbox::new("purity");
    // env --shell bash：幂等重建脚本（PATH 行含宿主 PATH，只断言结构 + 纯净性）
    let out = run_in(&fx, &["env", "--shell", "bash"], &[]);
    let text = norm(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    for ui_prefix in ["ℹ️", "✅", "⚠️", "🦀", "📋"] {
        assert!(!text.contains(ui_prefix), "env stdout 泄漏 UI 前缀 {ui_prefix}");
    }
    assert!(text.contains("_SDKM_BASE_PATH"), "env 脚本缺 base path 行");
    // 选中态 java 的 extra_vars 导出（未选中 SDK 的 keys 走 unset 行，逻辑对称）
    assert!(text.contains("export JAVA_HOME="), "env 脚本缺 extra_vars export 行");

    // use --shell：会话层赋值脚本（shell 语法随宿主检测，断言特征行 + 纯净性）
    let out = run_in(&fx, &["use", "java", "21", "--shell"], &[]);
    let text = norm(&fx, &out.stdout);
    assert_eq!(out.status.code(), Some(0));
    for ui_prefix in ["ℹ️", "✅", "⚠️", "🦀", "📋"] {
        assert!(!text.contains(ui_prefix), "use --shell stdout 泄漏 UI 前缀 {ui_prefix}");
    }
    assert!(text.contains("SDKM_ACTIVE_JAVA"), "use 脚本缺会话变量赋值行");
}

/// use 项目级 pin：写 .sdkm.toml 后 current 来源翻转为 project（端到端闭环）
#[test]
fn use_project_pin_end_to_end() {
    let fx = Sandbox::new("use_pin");
    let out = run_in(&fx, &["use", "java", "21"], &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = plain(&fx, &out.stdout);
    assert!(text.contains("Pinned `java` = `21.0.5`"));
    assert!(text.contains("Project config updated"));

    let pin = fs::read_to_string(fx.work().join(".sdkm.toml")).unwrap();
    assert_eq!(pin, "java = \"21.0.5\"\n");

    let out = run_in(&fx, &["current", "java"], &[]);
    assert!(plain(&fx, &out.stdout).starts_with("ℹ️  java 21.0.5 (project"));
}
