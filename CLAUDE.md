# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**sdkmate**（`sdkm`）是一款面向全栈工程师的跨平台 SDK 版本管理器，用 Rust 编写。通过符号链接切换 + 操作系统环境变量操纵，管理 Java、Node.js、Python、Maven、Go 等开发环境的版本。

## 构建与开发命令

```bash
cargo build                          # 调试构建
cargo build --release                # 发布构建（产出 sdkm 二进制）
cargo test                           # 运行所有测试（workspace 级别）
cargo test -p <crate>                # 运行指定 crate 的测试（cli / sdkcore / util）
cargo fmt                            # 格式化代码
cargo clippy --all-targets --all-features  # 代码检查
```

## 工作区架构

三成员 Cargo workspace + 根二进制包：

```
sdkmate (root) → 产出 sdkm 二进制，入口在 main.rs
  crates/cli    → CLI 层：clap 命令定义 + handler 实现
  crates/sdkcore → 核心业务逻辑：config、init、install、list、switch、version、env 操作、符号链接
  crates/util   → 共享工具：宏、SDK 类型、终端输出、配置辅助、路径
```

**依赖关系**：`cli → sdkcore, util`；`sdkcore → util`；`util` 无内部依赖。所有 crate 继承 workspace 元数据，均为 `publish = false`。

**sdkcore 内部**：`version/` 是版本解析公共模块（install/switch/list 共用），下载 URL 构建留在 `install/download_url`（install 专属）。各模块职责见源码文件，不在此赘述。

## 核心架构模式

### 版本切换机制
- 创建符号链接 `<symlink_dir>/<sdk_name>` → `<store>/<sdk>/<version>` 目录
- 将符号链接的 bin 目录加入 PATH
- 通过平台特定的环境变量操作设置额外变量（如 JAVA_HOME）
- 切换后更新 config.toml 中的 `current_version`

### 配置项 (`config.toml`)
```toml
[network]
proxy = ""                # HTTP proxy URL, e.g. "http://127.0.0.1:7890"
ssl_verify = true         # SSL certificate verification
connect_timeout = 30      # Connection timeout in seconds
cache_ttl_secs = 3600     # Version API cache TTL (seconds), 0 = always fetch
github_token = ""         # GitHub PAT for higher API rate limit

[[sdk]]
name = "java"
download_url = "..."   # 自定义 SDK 可省略（= 本地 switch-only，不远程安装）；内置 SDK 必填
bin_dir = "bin"
```

`symlink_dir` 为 `Option<String>`：`None` = 跟随 home（运行时 resolve 成 `<home>/links`），`Some` = 用户自定义。exe 移到哪 links 就在哪，绿色便携。

### 平台抽象
- `EnvOperation` trait：`WindowsEnvOperation`（注册表 + WM_SETTINGCHANGE 广播）和 `UnixEnvOperation`（shell profile 修改）
- `cfg(windows)` / `cfg(unix)` 条件编译用于环境操作、符号链接、默认路径
- 类型别名 `OsEnvOperation` 在编译时选择平台实现

### Home 目录发现
- sdkm 的 "home" = 运行中可执行文件的父目录（`current_exe()`），使工具可移植；可用 `SDKM_HOME` 环境变量覆盖（参考 rustup `RUSTUP_HOME`），未设则回退 `current_exe().parent()`
- Store 目录：`<home>/store/`；配置文件：`<home>/config.toml`（TOML + serde + 模板渲染）

### 模板渲染系统
- `TemplateRenderer` 解析 URL 和环境变量配置中的占位符：`{sdk_dir}`、`{sdkm_home}`、`{sdks_install_dir}`、`{os}`、`{arch}`、`{ext}`
- OS/arch 检测使用 `OnceLock` 静态变量；映射 macOS→darwin、x86_64→x64、aarch64→arm64

### 错误处理与输出
- 全项目使用 `anyhow::Result`（启用 backtrace 特性）；`anyhow::bail!` 和 `.context()` 处理错误
- 无自定义错误类型，不使用 thiserror（`BugReport` 标记类型除外——用于 CLI 层检测不可由用户解决的错误）
- 终端输出通过自定义宏：`info!`、`success!`、`warning!`、`error!`、`detail!`（crossterm 彩色，不是 `log` crate）
- CLI 错误：用 `error!` 宏打印；debug 构建额外显示 backtrace；检测 `BugReport` 标记时提示 GitHub issue URL；失败时 `process::exit(1)`

## CLI 命令结构

| 命令 | 别名 | Handler |
|------|------|---------|
| `sdkm init` | — | cli/InitHandler |
| `sdkm install <SDK> <VERSION>` | `i` | cli/InstallHandler |
| `sdkm uninstall <SDK> <VERSION>` | `rm`, `un` | cli/UninstallHandler |
| `sdkm list [SDK] [--remote] [--limit N]` | `ls`, `l` | cli/ListHandler |
| `sdkm switch <SDK> <VERSION>` | `s` | cli/SwitchHandler |
| `sdkm use <SDK> <VERSION> [--shell]` | — | cli/UseHandler |
| `sdkm env [SHELL]` | — | cli/EnvHandler |
| `sdkm hook [SHELL]` | — | cli/HookHandler |
| `sdkm current [SDK]` | `c` | cli/CurrentHandler |
| `sdkm config` | — | cli/ConfigHandler |
| `sdkm self uninstall` | — | cli/self_cmd::SelfUninstallHandler |
| `sdkm self update`（`--check`/`--rollback`） | `u` | cli/self_cmd::SelfUpdateHandler |

所有命令均已实现。每个命令在 `crates/cli/src/impls/` 中有 `CommandHandler` trait 实现，委托给 `crates/sdkcore/src/manager/` 中的 `SdkManager` 方法。

`config` 子命令：`set`/`get`/`list`/`delete`/`edit`/`add-sdk`/`remove-sdk`，按类型校验（`ValueType`：Url/UrlTemplate/Bool/U32/Path/Token/String）+ 原子写入（写入-重命名）+ 快照回滚；内置 SDK（java/node/python/maven）不可 delete/remove-sdk，只能 set 修改。

## 当前开发进度（2026-08-26）

### 本次改动 —— 导入 lint 工具化 + 三提交 review + shell 集成测试

承接上次（shell 后端双表 + fish，commit `03bcc8d`），本次三件事：

**1. 导入规则工具化（clippy `absolute_paths`）**——答用户「能否用 rustfmt/clippy 控制」：
- rustfmt **管不了**使用处全路径（`imports_granularity` 只管 use 聚合且 nightly-only）；clippy **可以**——`clippy::absolute_paths`（restriction，默认 allow）专查使用处绝对路径（不管 use 语句/宏/derive）。
- 配置三处：根 `Cargo.toml` 加 `[workspace.lints.clippy] absolute_paths = "warn"` + 各 crate `[lints] workspace = true` + 根 `clippy.toml` 的 `absolute-paths-max-segments = 3`。
- **max-segments=3 权衡**：只抓 4 段及以上超长全路径（`crate::shell::inject::fn`、`std::path::Path::new`），放过第三方惯用 3 段子模块写法（`tokio::fs::metadata`、`reqwest::header::AUTHORIZATION`、`crossterm::terminal::size`——强改 use 反而更糟）。max=2 误伤第三方、噪音过大；项目内 std 全路径已清为 use 短名。工具管不了的：相对路径「最多 1 级 `inject::fn()`」（absolute_paths 只管绝对路径）。详见记忆 `clippy-absolute-paths-lint`。
- 清理剩余使用处全路径：`util/terminal.rs` `std::process::Command::new` → use；`util/tests/test_bug_report.rs` 类型标注 `std::collections::HashMap` → use；`tests/{uninstall,self_uninstall}.rs` `std::sync::MutexGuard` → use；`tests/install.rs` 删孤儿 `use std::process`。

**2. Review 最近三提交 + 修复**（两子代理并行 review `f24bf48`/`03bcc8d`/`47e3343`，无致命 bug）：
- **[BUG 修复] `check_project_references`（uninstall.rs）**：原精确 `v == version` 比较，对模糊 pin（`"21"`，`use_project_version` 未装时写入或用户手写）永不命中 → 卸载该版本时**漏发项目引用警告**。改为 `fuzzy_match_version_core(&[version], pin).is_ok()`（pin 当输入、被卸载版本当候选池）。
- **[NIT] `Shell` 枚举加 `#[repr(u8)]`**：`hook_cache` 用 `shell as u8` 存盘，把判别序契约写死（Bash=0/Zsh=1/Fish=2/PowerShell=3）。
- **[NIT] `generate_env_script_inner` 去掉 `_pwd` 误导形参**：cache key 由 `generate_env_script_cached` 持有 pwd，inner 用 `find_project_config()`（同进程 current_dir 一致）。
- **review 不成立**：`resolve_local_version`「吞 fuzzy Err」——`fuzzy_match_version_core` 的 Err 只有「无匹配」一种（前缀匹配取最新，不报歧义），Err 即未装，当 None 处理正确，不改。
- **遗留 RISK（未改）**：`SDKM_ACTIVE_*` 只有 set 路径无 unset 入口（清除靠手敲 `unset`），可用性缺口非 bug，待加 `use --shell --unset` 或 help 说明。

**3. shell 后端核心入口集成测试**（新文件 `crates/sdkcore/tests/shell_integration.rs`，7 测全过）：
- `use_session_version` 四 shell 端到端（模糊 "21" → 精确 21.0.2+9，各 shell 赋值前缀）+ 未装 bail
- `generate_env_script_cached` 端到端（有项目 pin → 含 bin PATH 行 + export JAVA_HOME；无配置 → unset JAVA_HOME 幂等还原；缓存幂等）
- `HookCache` 跨 shell 串扰（put bash → resolve fish/zsh/PS miss）+ 旧 schema miss
- 均跨平台入口，cfg(unix) 路径 Windows 也能编译（测试只走公共入口，不直接引 unix-only fn）

**新增文件**（1）：`crates/sdkcore/tests/shell_integration.rs`
**改动文件**：根 `Cargo.toml`、`clippy.toml`、三 crate `Cargo.toml`、`util/{terminal,shell}.rs`、`util/tests/test_bug_report.rs`、`sdkcore/tests/{install,uninstall,self_uninstall}.rs`、`sdkcore/src/{uninstall,shell/env}.rs`。**未发版**。

### 沿自上次（shell 后端双表 + fish 支持）—— 仍有效，排查必读

**fish 关键铁律**（细节见 `shell_backend/fish.rs` 注释 + `hook.rs`/`inject.rs` 测试守护）：
1. 一律 `| source`，禁 `eval (...)`（命令替换按换行拆参、eval 压行）
2. `_SDKM_BASE_PATH` 是 list：base 引用不引号、bin 单独引号
3. `set -e` 对不存在变量报错：unset 行必须 `set -q K; and set -e K` 守卫
4. PATH 持久化用 `fish_add_path --path "<dir>"`（必须 `--path`，否则写 universal `fish_user_paths`）
5. source_profile 输出必须 `string join : $PATH`（`$PATH` 是 list）

**关键机制**：hook 每提示符触发 `sdkm env`（热更新 + mtime 缓存）；幂等重建（base 一次性存 PATH，env 永远重建 + 未选中 known keys unset）；stdout 纯净（脚本走 stdout，诊断走 stderr）；未装降级（项目/会话版本缺失 → stderr warn + 跳过回退全局）；PowerShell 注入铁律（`-join [Environment]::NewLine`、ASCII、Documents 重定向、IEX 非 scriptblock）。Shell 职责分层（`util::shell` 枚举+detect/parse + `shell_backend` 双表 + `sdkcore::shell` 编排 + `env/unix.rs` backend 化 + `hook_cache` shell 字段 schema=3）见源码。

**注：下次更新进度时，删除本条（只保留最新一次会话）。**

## 已知问题与注意事项

- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- **Windows 需管理员运行**：环境变量与 PATH 写入 `HKEY_LOCAL_MACHINE`，符号链接创建需 `SeCreateSymbolicLinkPrivilege`（管理员或开发者模式），`init`/`switch` 需管理员权限
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB（功能生效，彻底修复需 shim 模式重构或用户手动 source）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.92.0（edition 2024）
- **zip 仅启用 `deflate` 特性**（体积优化）：若日后解压非 deflate 压缩或密码 zip 会失败，按需在 `Cargo.toml` 加回对应 feature（`bzip2`/`lzma`/`zstd`/`deflate64`/`aes-crypto`）
- **reqwest 0.13.4 + rustls**：`default-features = false, features = ["json","gzip","default-tls"]`。`default-tls = rustls`（aws-lc-rs + rustls-platform-verifier 走系统 CA bundle），不依赖系统 OpenSSL——Linux 编译只需 `cmake`，运行时需 `ca-certificates`。禁 `http2`/`charset`/`query`/`form`
- **BugReport 标记只用于真正不可由用户修复的错误**：`install_sdk` 入口用 `?` 传播，真正的 bug（解压/校验失败、内置配置缺失）在 `install_sdk_async` 内部用 `try_bug!`/`bail_bug!` 精确标记，CLI 层 `needs_bug_report` 靠 `downcast_ref` 检测。用户取消是普通 `bail!`。switch 的 `try_step!` 不标 BugReport——步骤失败（symlink 创建 os error 1314、env 写入 os error 5）多属权限/环境问题，用户可自行解决（管理员运行），返回普通错误。**init 第 4 步 symlink_dir 不可写属用户环境问题，用 `bail!` 不用 `try_bug!`**
- **cfg(unix) 代码在 Windows 上不编译，类型错误会漏到 Linux 才暴露**：`env/unix.rs` 整个模块 `#[cfg(unix)]`。改 `unix.rs` 后**必须**验证类型——Windows 上临时把 `env/mod.rs` 的 `#[cfg(unix)] mod unix;` 改成 `mod unix;`（注释掉对应 `pub use` 避免冲突），跑 `cargo check -p sdkcore` 强制编译暴露错误，验证完改回。`cargo check --target x86_64-unknown-linux-gnu` 不行（aws-lc-rs build.rs 找不到 `x86_64-linux-gnu-gcc`）；或直接 WSL `cargo build`。**filter 闭包参数是 `&Self::Item`（item 的引用）**——`split()` 的 item 是 `&str`，闭包参数是 `&&str`，必须用 `|&p|` 解构才能跟 `&str` 比较；`&String` 用 `.as_str()` 转 `&str`
- **SDKM_HOME 环境变量可覆盖 home**：`util/path.rs::get_sdkm_home` 优先读 `SDKM_HOME`，未设回退 `current_exe().parent()`，行为兼容。主要解锁 in-process 集成测试——`SdkManager::new()` 等路径全经 `get_sdkm_home`（绑 `current_exe`，原本不可注入），注入后测试可全部指向临时目录。`tests/uninstall.rs` 等用：设 `SDKM_HOME=temp` + 手工构造 `SdkManager{config, env_operation: Box<MockEnv>}`（字段 pub，`MockEnv` impl `EnvOperation` 记录调用）+ 模块级 `Mutex` 串行（`set_var` 全局竞态）+ `TestEnv` Drop 恢复 env/清理目录
- **改完代码先 review 再交付**（编译通过 ≠ 代码干净）：每次改完、build/test/通知用户/提交前，对本次 diff 逐文件自审——(1) 逻辑/边界 bug（空集、None、并发、`panic=abort` 下后台线程无 unwrap）；(2) 因改动变孤儿的 import/变量/函数一并清；(3) 风格违规（标准库别用 `std::fs::` 全路径，优先 `use` 短名，歧义才加包名）；(4) 冗余/可简化处。复核通过再 build/test/交付

## 发布流程

工作流 `.github/workflows/release.yml` 在每次 push 到 master 时跑 tag job，但**只有版本号 bump 才会真正发版**——版本号对应 tag 已存在则跳过 build/release。不是每次提交都发版。

### 发版三步
1. 改根 `Cargo.toml` 的 `[workspace.package] version` 一行
2. 本地 `cargo build` 刷新 `Cargo.lock`（CI 用 `--locked`，必须同步否则构建失败）
3. 单独 commit bump（`chore: release vX.Y.Z`）+ push 到 master → 工作流自动打 tag、五产物构建、创建 release

### 要点
- **changelog 自动生成**：来自「上个 tag..HEAD」的 conventional commits，按 `feat`/`fix`/`refactor`/`docs`/`ci` 等前缀分类（`.github/scripts/gen_changelog.sh`）。保持约定式 commit message
- **根 `CHANGELOG.md` 由 CI 维护**：发版后 upload-release job 把版本正文 prepend 到根 `CHANGELOG.md` 并 commit 回 master。**本地不要手改**，会被下次发版覆盖；GITHUB_TOKEN push 不触发 workflow 递归
- **版本号只能递增、不可复用**：已发版本 tag 已存在，再 push 同版本会被跳过。重发同版本需先删 tag+release 再 push；或直接 bump 到下一版本（推荐）
- **不发 crates.io**：所有 crate `publish = false`，发布物为 GitHub Release 上五产物二进制（Linux gnu/musl、macOS ARM/Intel、Windows）
- **五产物矩阵**：`ubuntu-22.04`×{gnu,musl} + `macos-14`/aarch64 + `macos-14` 交叉编译 x86_64-apple-darwin + `windows-latest`。Linux gnu 用 22.04（glibc 2.35）避开 24.04 的 `GLIBC_2.39` 墙；musl 全静态通吃老系统/Alpine；macOS 必须设 `MACOSX_DEPLOYMENT_TARGET=11.0`（否则 runner SDK 升高会编出老 macOS 跑不了的二进制）；Windows 保持 latest
- **纯 CI/文档修复复用原版本号删 tag 重发，patch/minor bump 只留给代码变化**
- **本地 origin 指向 gitee，GitHub remote 叫 `github`**：查 GitHub 状态用 API 或 `git fetch github --tags`

## 提交规范

- 格式：`type: description`（如 `feat: add switch command`、`fix: resolve xxx issue`）
- 类型：feat、fix、docs、refactor、test、chore
- 分支命名：`feature/xxx`、`fix/xxx`、`docs/xxx`
