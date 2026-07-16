# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**sdkmate**（`sdkm`）是一款面向全栈工程师的跨平台 SDK 版本管理器，用 Rust 编写。通过符号链接切换 + 操作系统环境变量操纵，管理 Java、Node.js、Python、Maven、Rust 等开发环境的版本。

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
- 将符号链接的 bin 目录加入操作系统 PATH
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
| `sdkm current [SDK]` | `c` | cli/CurrentHandler |
| `sdkm config` | — | cli/ConfigHandler |
| `sdkm self uninstall` | — | cli/self_cmd::SelfUninstallHandler |

所有命令均已实现。每个命令在 `crates/cli/src/impls/` 中有 `CommandHandler` trait 实现，委托给 `crates/sdkcore/src/manager/` 中的 `SdkManager` 方法。

`config` 子命令：`set`/`get`/`list`/`delete`/`edit`/`add-sdk`/`remove-sdk`，按类型校验（`ValueType`：Url/UrlTemplate/Bool/U32/Path/Token/String）+ 原子写入（写入-重命名）+ 快照回滚；内置 SDK（java/node/python/maven）不可 delete/remove-sdk，只能 set 修改。

## 当前开发进度（2026-07-16）

### 本次改动 —— `sdkm self update` 自更新子命令

新增 `sdkm self update`，从 GitHub Release 自检最新版并就地替换 sdkm 二进制，带备份+验证+回滚兜底。路径 `self update`，与 `self uninstall` 对称。

**命令形态**（`cli/impls/self_cmd.rs` 加 `SelfSub::Update` + `SelfUpdateHandler`）：
- `sdkm self update`（别名 `u`）—— 查 GitHub latest，**只升不降**（远程 ≤ 当前 → "already up to date" 退出，不下载）
- `sdkm self update --check` / `-c` —— 只打印 current vs latest，不下载（只读）
- `sdkm self update --rollback` / `-r` —— 把 `.bak` 恢复为当前二进制（本地，不联网）
- `--check` 与 `--rollback` 互斥（同时给则 bail）。非破坏性，不强制确认（与 uninstall 区别）。`long_about` 详述备份/验证/回滚机制

**core 模块**（新 `sdkcore/src/self_update.rs`，与 `self_uninstall.rs` 并列；`lib.rs` 加 `pub mod self_update;`）：
1. **版本源**：GitHub API `releases/latest`，serde 解析 `tag_name` + `assets[].browser_download_url`（单请求拿版本+URL）。复用 `install::downloader::build_reqwest_client`（proxy/ssl_verify/github_token/timeout 全自动生效）
2. **平台→asset 映射**：编译期 `cfg!`（= 已发布二进制的 target，非开发机；同 target 产物必能在同 target 二进制环境跑，与 rustup macOS `from_build()` 一致）。5 产物：windows-x86_64.zip / macos-{aarch64,x86_64}.tar.gz / linux-x86_64-{gnu,musl}.tar.gz（`target_env` 分流 gnu/musl）。无产物平台编译期 bail。当前矩阵下无需运行时检测
3. **版本比较**：手写 `parse_ver` 三段 u64（不加 semver 依赖）。当前版本 `env!("CARGO_PKG_VERSION")`
4. **备份/临时副本全放 work_dir**（`<home>/.tmp/self_update/`，`work_dir()` helper）：`.bak`/`.discard`/`.bad` 都在此，**exe 同目录保持干净**（不污染 sdkm 安装目录）。exe 父目录只留 sdkm 二进制本身
5. **替换+回滚**（参考 rustup `self_update`，单二进制折中）：
   - **权限预检**（仿 `self_update_permitted`）：在 exe 父目录试写，碰网络前挡 `PermissionDenied`
   - **全程 rename 不 delete**：Windows 允许 rename running exe（MoveFileEx 改目录条目），不允许 delete running exe——self_uninstall 注释的"running exe 自删不可靠"正指 delete，本方案避开。`replace_binary`：`clean_leftovers` → 删旧 .bak → `rename(exe, work/.bak)` → `rename(new_exe, exe)` → spawn `exe --version` 验证 → 失败自动 `rollback_to_bak`
   - **`--rollback`**：备份不存在 → bail"no backup; run `sdkm self update` first"（不允许回滚）。有备份：`rename(exe, work/.discard)` → `rename(work/.bak, exe)` → spawn 验证；.bak 损坏则用 `work/.bad` 中转恢复原 exe
   - **Windows 固有限制**：rollback 成功后 `work/.discard`（原 running exe 副本）删不掉（进程仍占用），留 work_dir（不污染 exe 目录）。`clean_leftovers(work, name)` 在每次 update/rollback 开头清上次残留的 `.discard`/`.bad`（rustup "下次运行清理" 范式）。`.bak` 保留供 rollback，不自动删
   - **跨盘注意**：`work_dir` 在 `<home>/.tmp`，若 SDKM_HOME 与 exe 不同盘，rename running exe 跨盘失败（Windows 跨盘 rename = copy+delete，delete running 失败）→ bail 提示"keep SDKM_HOME on the same drive as sdkm.exe"。默认 home=exe parent 同盘无此问题
6. 复用 `install::downloader::download_with_retry`（断点续传+重试）+ `install::extractor::extract_archive`（zip/tar.gz）。indicatif 自带进度条（不复用 install 的 SDK 文案模板）。下载/解压临时文件用完即删，`.bak` 留 work_dir

**错误处理**：全程 `anyhow`+`.context()`；网络/权限/平台/已是最新/无备份/验证失败 → 普通 `bail!`（用户可解决，非 bug），**不用 `BugReportError`**（与 CLAUDE.md「BugReport 只用于不可修复错误」一致）。`--version` 在 clap parse 阶段处理不读 config（`main.rs` 确认），spawn 验证可靠。中文注释。

**解耦**：只新增模块 + self_cmd variant，不碰 install/switch/list/uninstall/size_cache 任一行；不新增依赖/配置；不碰 `main.rs`/`lib.rs`（cli 侧 `Commands::Self_` 已统一转 `SelfHandler::run`）。

**实测**（隔离临时 home + SDKM_HOME，不碰真实环境）：构造 0.2.7 旧版 → `--check` 显示 update available ✓；`self update` 下载替换 `--version` 变 0.2.8 ✓（.bak 在 work_dir、exe 目录无残留）；`--rollback` 0.2.8→0.2.7 ✓；无备份 `--rollback` bail"no backup" ✓；别名 `self u -c` ✓；Windows running exe 副本 `.discard` 删不掉 → 留 `<home>/.tmp/self_update/`（exe 目录干净）→ 下次 `clean_leftovers` 清理 ✓（rustup 同款行为）。release 覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（备份 .bak），真实 home `--check` already up to date ✓。未发版。

**注：下次更新进度时，删除本条（只保留最新一次会话）。**

## 已知问题与注意事项

- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- Windows 环境变量操作写入 `HKEY_LOCAL_MACHINE`（需要管理员权限），非 `HKEY_CURRENT_USER`
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB（功能生效，彻底修复需 shim 模式重构或用户手动 source）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.92.0（edition 2024）
- **zip 仅启用 `deflate` 特性**（体积优化）：若日后解压非 deflate 压缩或密码 zip 会失败，按需在 `Cargo.toml` 加回对应 feature（`bzip2`/`lzma`/`zstd`/`deflate64`/`aes-crypto`）
- **reqwest 0.13.4 + rustls**：`default-features = false, features = ["json","gzip","default-tls"]`。`default-tls = rustls`（aws-lc-rs + rustls-platform-verifier 走系统 CA bundle），不依赖系统 OpenSSL——Linux 编译只需 `cmake`，运行时需 `ca-certificates`。禁 `http2`/`charset`/`query`/`form`
- **BugReport 标记只用于真正不可由用户修复的错误**：`install_sdk` 入口用 `?` 传播，真正的 bug（解压/校验失败、内置配置缺失）在 `install_sdk_async` 内部用 `try_bug!`/`bail_bug!` 精确标记，CLI 层 `needs_bug_report` 靠 `downcast_ref` 检测。用户取消是普通 `bail!`。switch 的 `try_step!` 同理。**init 第 4 步 symlink_dir 不可写属用户环境问题，用 `bail!` 不用 `try_bug!`**
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
