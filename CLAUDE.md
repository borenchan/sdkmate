# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**sdkmate**（`sdkm`）是一款面向全栈工程师的跨平台 SDK 版本管理器，用 Rust 编写。通过符号链接切换 + 操作系统环境变量操纵，管理 Java、Node.js、Python、Maven、Go 等开发环境的版本。

## 构建与开发命令

```bash
cargo build                          # 调试构建
cargo build --release                # 发布构建（产出 sdkm 二进制）
cargo test                           # 运行所有测试（workspace 级别）
cargo test -p <crate>                # 运行指定 crate 的测试（sdkm-cli / sdkm-core / sdkm-util）
cargo fmt                            # 格式化代码
cargo clippy --workspace --all-targets --all-features  # 代码检查（含 GUI）
```

## 工作区架构

四成员 Cargo **virtual workspace**（根目录无包；`default-members = ["crates/sdkm-cli"]`，裸 `cargo build` 只构建 CLI，不连带 GUI）：

```
crates/sdkm-cli    → CLI 前端 + sdkm 二进制入口：src/main.rs（[[bin]] sdkm，autobins=false）
                  build.rs（Windows exe 图标）；tests/cli_golden.rs（CLI 集成金样）
                  clap 命令定义 + handler 实现 + terminal_sink（事件适配器）
                  + terminal.rs/macros.rs（终端渲染原语与输出宏）
  crates/sdkm-core → 核心业务（纯数据 API）：config、init、install、list、switch、version、
                   env 操作、符号链接；report.rs（事件契约）
  crates/sdkm-util   → 共享工具：SDK 类型、配置辅助、路径、shell backend、诊断数据（diagnostics）
```

**依赖关系**：`sdkm-cli → sdkm-core, sdkm-util`；`sdkm-core → sdkm-util`；`sdkm-util` 无内部依赖；`sdkm-gui → sdkm-core, sdkm-util`。所有 crate 继承 workspace 元数据，均为 `publish = false`。

**分层铁律（GUI 铺路，2026-11-06 落地）**：
- **sdkm-core 零打印、零 stdin、零终端渲染依赖**——过程消息经 `sdkm_core::report`（`CoreEvent` / `EventSink` / `ProgressKind` 7 场景 / `ProgressHandle` Drop 兜底 / `NullSink`）通知前端；命令结果走结构化返回值；最终失败走 `Result::Err`（core 不对同一失败再发 Error 事件）。文案归 core（事件说什么），样式归前端（TerminalSink 怎么渲染）
- **`SdkManager { config, env_operation, sink: Arc<dyn EventSink> }`**：每操作独立构造 Manager+Sink（sdkm-cli 经 `terminal_sink::manager()`），串行执行，无全局 EventBus
- **runtime 归前端持有（2026-11-16）**：core 只出 `*_async` API（全部 `pub`，无同步壳）；CLI 用 `crates/sdkm-cli/src/runtime.rs` 的进程级单线程 runtime 在阶段边界 block_on；GUI 用 `crates/sdkm-gui/src/data.rs` 的进程级 2-worker runtime `spawn` 调度。`SdkManager`/`EventSink`/`ProgressHandle`/`EnvOperation` 均 `Send + Sync`
- **长阻塞段统一走 `sdkm_core::runtime::run_blocking`**（内部 `spawn_blocking`）：解压/目录调整/校验/清理/卸载删除共 8 处；`switch` 因 `&mut self` 且毫秒级保留内联。全项目只有 `runtime.rs` 知道阻塞怎么跑，将来换运行时只改这一个文件
- **HTTP client 进程级缓存**（`downloader.rs`，`OnceLock` + network 配置指纹）：连接池绑定创建它的 runtime，故必须前端持有常驻 runtime 后才生效
- **确认交互归调用端**：core 零 stdin；`resolve_install_version`/`resolve_installed_version`/`resolve_uninstall_target`/`check_init_env` 产出决策数据，sdkm-cli prompt / GUI 弹窗确认后调执行 API；执行段仍自查关键状态（不信任预检查结果）
- **EnvOperation 不感知 UI**：写方法返回 `Vec<EnvAction>` 结构化动作，业务层经 `report_env_actions` 转事件
- **util 不含终端渲染**：颜色/光标/表格/prompt/输出宏归 sdkm-cli（`sdkm_cli::terminal` + `sdkm_cli::macros`）；sdkm-util 只留通用原语（`sdkm_util::diagnostics` 收 os_version/platform_info/build_bug_report_url）。`cargo tree -p sdkm-core -p sdkm-util` 验证 GUI 依赖链零 crossterm/unicode-width/indicatif

**sdkm-core 内部**：`version/` 是版本解析公共模块（install/switch/list 共用），下载 URL 构建留在 `install/download_url`（install 专属）。各模块职责见源码文件，不在此赘述。

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
- **输出分层**：sdkm-core 零打印，过程消息经 `SdkManager.sink`（`sdkm_core::report` 事件契约）发前端；cli 直接输出用 `crate::{info!, success!, warning!, error!, detail!}` 宏（定义在 `cli/src/macros.rs`，渲染在 `cli/src/terminal.rs`，crossterm 彩色，不是 `log` crate）
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
| `sdkm env [--shell SHELL]` | — | cli/EnvHandler |
| `sdkm hook [SHELL]` | — | cli/HookHandler |
| `sdkm current [SDK]` | `c` | cli/CurrentHandler |
| `sdkm config` | — | cli/ConfigHandler |
| `sdkm self uninstall` | — | cli/self_cmd::SelfUninstallHandler |
| `sdkm self update`（`--check`/`--rollback`） | `u` | cli/self_cmd::SelfUpdateHandler |

所有命令均已实现。每个命令在 `crates/sdkm-cli/src/impls/` 中有 `CommandHandler` trait 实现，委托给 `crates/sdkm-core/src/manager/` 中的 `SdkManager` 方法。

`config` 子命令：`set`/`get`/`list`/`delete`/`edit`/`add-sdk`/`remove-sdk`，按类型校验（`ValueType`：Url/UrlTemplate/Bool/U32/Path/Token/String）+ 原子写入（写入-重命名）+ 快照回滚；内置 SDK（java/node/python/maven）不可 delete/remove-sdk，只能 set 修改。

## 当前开发进度（2026-11-17）

> **维护规则**：本节只保留最新一次改动，每次完成后**整体替换**（不是追加），文件体积不随历史增长。

### 批次：Tomcat 内置 + Java 多供应商

- **Tomcat（#19）**：新增 `BuiltinSdk::Tomcat` + 种子（版本源 `archive.apache.org` 全历史；下载主源 dlcdn、备源 archive，可 `config set sdk.tomcat.download_url` 换镜像）。版本发现写在 `version/discovery.rs`（不新建模块）：root 索引 → `tomcat-9/10/11` 分支多请求，复用 `fetch_version_data` 按分支缓存；`vX.Y.Z/` 严格匹配天然排除里程碑；`feature_version` 取版本首段。install/list 各自专属分支，下载走模板渲染 `{feature_version}/{version}/{ext}`；`get_version_discovery` 返回防御占位。
- **add_to_path 通用开关**：`SdkConfig`/`SdkSeed`/keys/validation 新增（默认 true，仅 false 落盘）；`switch`、hook 全局层、项目层三处都遵守。Tomcat 设 false：切换只建软链（current 落点）+ 设 `CATALINA_HOME`，不写 PATH、不做冲突检测，提示「`tomcat` is not added to PATH. Launch it from: <links>/tomcat/bin」。
- **Java 多供应商（#20）**：sdkman 式后缀 4 家（Temurin 无后缀 / `-zulu` / `-amzn` / `-librca`）；输入支持 `21-zulu` 与 `zulu@21` 两种等价写法（`fuzzy.rs` 的 `parse_java_version_input`/`format_java_version`/`fuzzy_match_version_with_vendor`）。`discovery.rs` 四家抓取：Adoptium feature_releases（Temurin，裸大版本仍走 config 模板，保留镜像覆盖）、Azul（latest+history 合并、fx/crac/musl 过滤）、Corretto（GitHub tags + `macosx` 命名 + Windows aarch64 明确报错）、Liberica（`arch=x86/arm`、去 `+build`）。`list java -r` 四家 LTS+最新 feature 合并（`tokio::spawn` 并行 + 缓存），信息源主备读 config，双源失败降级默认 LTS；本地匹配 6 处统一走 `match_installed_version`。
- **下载器健壮性**：4xx（除 429/408）确定性失败不重试、直接切备源；30 秒无数据判定卡死并中止（此前要等 600s 总超时）。
- **测试**：新增 Java 输入/匹配、四家解析、Tomcat 目录解析、add_to_path、Tomcat 全链路（本地 HTTP + CATALINA_HOME + 不进 PATH 提示）、Java 目录隔离、404 不重试 / 500 重试；CLI 金样加 Tomcat 离线列表与 java 后缀本地列表。
- **实测（2026-11-17）**：`list tomcat -r` 205 条（全历史）；`list java -r` 515 条（四家混排）；沙箱安装 tomcat 11.0.29 / 11.0.6、`zulu@17` → `17.0.21-zulu`。镜像测速：华为云 5.9MB/s、阿里云 3.1、清华 0.6、dlcdn 卡死（20s 无数据）。
- **选型（2026-11-17）**：Java 首批 4 家（均有稳定 JSON API）；Oracle 不收录（NFTC 许可、sdkman 客户端亦不负责下载）；Flutter 本期不做（用户已提 PR）。

## 已知问题与注意事项

- **Tomcat 分支白名单硬编码**：`TOMCAT_SUPPORTED_FEATURES = [9,10,11]`（`version/discovery.rs`），Apache 新增分支（如 12）需手动加入；版本列表解析的是 HTML 目录索引（自定义 SDK 引擎仍只认 JSON）
- **Java 供应商 API 均为单源**（无可换镜像）：只有 Adoptium 信息源（major 列表）支持 config 主备，其余四家抓取无备源；Temurin 只有「默认供应商 + 裸大版本」走 `sdk.java.download_url` 模板（可换 Adoptium 镜像），精确版本与其他供应商用各家 API 直链
- **Tomcat 默认下载源 dlcdn 国内可能卡死**（实测 20s 零数据）：文档（`docs/usage.md`、`skills/SKILL.md`）给华为云镜像一行命令；下载器已加「4xx 不重试直接切备源 + 30s 无数据判卡死」
- **使用处禁止跨模块全路径，一律顶部 use 短名**（如 `sdkm_util::builtin::find_seed(...)` 写体内 → 改 `use sdkm_util::builtin::find_seed;`）。rustfmt 只管 use 语句、管不了使用处写法，无自动化工具；clippy `absolute_paths` 的 `max-segments=3`（为放过 `tokio::fs` 等第三方 3 段惯用写法，实测降 2 噪音比 4:1）恰好抓不到项目内 3 段路径——**review diff 时人工扫 `sdkm_util::`/`crate::` 开头的体内全路径**（宏内 `$crate::` 除外，那是正确写法）。CLI help 的内置 SDK 名单固定写 "java, node, python, go, maven, ... (see docs)" 省略号形式，不随名单扩容联动修改
- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- **Windows 需管理员运行**：环境变量与 PATH 写入 `HKEY_LOCAL_MACHINE`，符号链接创建需 `SeCreateSymbolicLinkPrivilege`（管理员或开发者模式），`init`/`switch` 需管理员权限
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB（功能生效，彻底修复需 shim 模式重构或用户手动 source）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.98（edition 2024；gpui-kit 0.9 依赖 slice_as_array，需 stable 1.97+）
- **zip 仅启用 `deflate` 特性**（体积优化）：若日后解压非 deflate 压缩或密码 zip 会失败，按需在 `Cargo.toml` 加回对应 feature（`bzip2`/`lzma`/`zstd`/`deflate64`/`aes-crypto`）
- **reqwest 0.13.4 + rustls**：`default-features = false, features = ["json","gzip","default-tls"]`。`default-tls = rustls`（aws-lc-rs + rustls-platform-verifier 走系统 CA bundle），不依赖系统 OpenSSL——Linux 编译只需 `cmake`，运行时需 `ca-certificates`。禁 `http2`/`charset`/`query`/`form`
- **BugReport 标记只用于真正不可由用户修复的错误**：`install_sdk` 入口用 `?` 传播，真正的 bug（解压/校验失败、内置配置缺失）在 `install_sdk_async` 内部用 `try_bug!`/`bail_bug!` 精确标记，CLI 层 `needs_bug_report` 靠 `downcast_ref` 检测。用户取消是普通 `bail!`。switch 的 `try_step!` 不标 BugReport——步骤失败（symlink 创建 os error 1314、env 写入 os error 5）多属权限/环境问题，用户可自行解决（管理员运行），返回普通错误。**init 第 4 步 symlink_dir 不可写属用户环境问题，用 `bail!` 不用 `try_bug!`**
- **cfg(unix) 代码在 Windows 上不编译，类型错误会漏到 Linux 才暴露**：`env/unix.rs` 整个模块 `#[cfg(unix)]`。改 `unix.rs` 后**必须**验证类型——Windows 上临时把 `env/mod.rs` 的 `#[cfg(unix)] mod unix;` 改成 `mod unix;`（注释掉对应 `pub use` 避免冲突），跑 `cargo check -p sdkm-core` 强制编译暴露错误，验证完改回。`cargo check --target x86_64-unknown-linux-gnu` 不行（aws-lc-rs build.rs 找不到 `x86_64-linux-gnu-gcc`）；或直接 WSL `cargo build`。**filter 闭包参数是 `&Self::Item`（item 的引用）**——`split()` 的 item 是 `&str`，闭包参数是 `&&str`，必须用 `|&p|` 解构才能跟 `&str` 比较；`&String` 用 `.as_str()` 转 `&str`。**涉及跨平台代码提交前必须保证双平台编译通过**（v0.5.1 教训：region.rs unix 分支漏 `-> bool`，tag 构建全挂，删 tag 重发补救）；单文件级验证法 = 临时剥 `#[cfg(unix)]` 属性跑 `cargo check`，完事必须恢复属性并重编译确认（sed 批量剥极易漏恢复）
- **SDKM_HOME 环境变量可覆盖 home**：`util/path.rs::get_sdkm_home` 优先读 `SDKM_HOME`，未设回退 `current_exe().parent()`，行为兼容。主要解锁 in-process 集成测试——`SdkManager::new()` 等路径全经 `get_sdkm_home`（绑 `current_exe`，原本不可注入），注入后测试可全部指向临时目录。`tests/uninstall.rs` 等用：设 `SDKM_HOME=temp` + 手工构造 `SdkManager{config, env_operation: Box<MockEnv>}`（字段 pub，`MockEnv` impl `EnvOperation` 记录调用）+ 模块级 `Mutex` 串行（`set_var` 全局竞态）+ `TestEnv` Drop 恢复 env/清理目录。**沙箱测试注意：config 里的 `symlink_dir` 若是绝对路径会写真实 links 目录，测试必须同时隔离该配置**
- **改完代码先 review 再交付**（编译通过 ≠ 代码干净）：每次改完、build/test/通知用户/提交前，对本次 diff 逐文件自审——(1) 逻辑/边界 bug（空集、None、并发、`panic=abort` 下后台线程无 unwrap）；(2) 因改动变孤儿的 import/变量/函数一并清；(3) 风格违规（标准库别用 `std::fs::` 全路径，优先 `use` 短名，歧义才加包名）；(4) 冗余/可简化处。复核通过再 build/test/交付
- **env/unix.rs 的 RebuildLine 与 PerDirCommand 是两条独立代码路径**：改 PATH 持久化行为时（插入位置、去重、删除），bash/zsh 走 RebuildLine 分支、fish 走 PerDirCommand 分支，**修一处必须检查另一处是否同型遗漏**——bash Bug 2（PATH 行错位到 hook 后）修了 RebuildLine、漏了 PerDirCommand，fish 用户 WSL 实测才暴露（v0.4.3 补修）
- **bug 回归测试用临时文件验证后删除，不提交**：用户规矩——测试代码跑通证明修复生效即可，diff 里只留修复本身
- **WSL + Windows 双装 sdkm 的跨平台污染**（排查"项目级切换重启失效"教训）：WSL 默认 `appendWindowsPath=true` 继承 Windows PATH，但 **WSL bash 不补 `.exe` 后缀**（[WSL#2003](https://github.com/microsoft/WSL/issues/2003)）——bare `sdkm`/`node` 只命中文件名恰好叫 `sdkm`/`node`（无后缀）的条目，Windows 目录里的 `sdkm.exe`/`node.exe` **不匹配**。所以 Windows PATH 里的 `/mnt/d/.../sdkm` 是纯干扰，不能让 bare 命令生效。用户若在 WSL 敲 `sdkm.exe`（带后缀）则跑的是 **Windows sdkm**，操作 Windows home（`D:\...\sdkm`）、写 PowerShell profile/注册表，**完全不碰 Linux `.bashrc`**。排查要点：让用户 `command -v sdkm` + `file $(command -v sdkm)` 确认命中 Linux ELF 还是 Windows PE。`init` 的 PATH 持久化靠 `add_sdk_path`→`add_path_entry` 写 profile 行，但 `source_profile` 只把新 PATH 设进 **sdkm 子进程**（`env::set_var`），**不回传交互 shell**——所以 init/switch 后必须新开 shell 或 `source ~/.bashrc` 才生效。已修三 bug：dedup 不再回退进程 `$PATH`（Bug 1）、新建 PATH 行插到 hook 行之前（Bug 2，避免 source 时 hook 找不到 sdkm）、fish PerDirCommand 分支同型错位 relocate 自愈（v0.4.3）。详见进度段本次改动。

## 发布流程

工作流 `.github/workflows/release.yml` 在每次 push 到 master 时跑 tag job，但**只有版本号 bump 才会真正发版**——版本号对应 tag 已存在则跳过 build/release。不是每次提交都发版。

### 发版三步
1. 改根 `Cargo.toml` 的 `[workspace.package] version` 一行
2. 本地 `cargo build` 刷新 `Cargo.lock`（CI 用 `--locked`，必须同步否则构建失败）
3. 单独 commit bump（`chore: release vX.Y.Z`）+ push 到 master → 工作流自动打 tag、七产物构建、创建 release

### 要点
- **changelog 自动生成**：来自「上个 tag..HEAD」的 conventional commits，按 `feat`/`fix`/`refactor`/`docs`/`ci` 等前缀分类（`.github/scripts/gen_changelog.sh`）。保持约定式 commit message
- **release body 中英双语**：upload-release 里 `translate_changelog_zh.py` 走谷歌翻译免费网页端点（`client=gtx`，无需 key）只翻 commit 描述，分组标题查固定映射表，反引号/专名占位符保护；翻译失败/限流自动兜底纯英文，不阻塞发版。根 `CHANGELOG.md` 仍纯英文
- **根 `CHANGELOG.md` 由 CI 维护**：发版后 upload-release job 把版本正文 prepend 到根 `CHANGELOG.md` 并 commit 回 master。**本地不要手改**，会被下次发版覆盖；GITHUB_TOKEN push 不触发 workflow 递归
- **版本号只能递增、不可复用**：已发版本 tag 已存在，再 push 同版本会被跳过。重发同版本需先删 tag+release 再 push；或直接 bump 到下一版本（推荐）
- **不发 crates.io**：所有 crate `publish = false`，发布物为 GitHub Release 上七产物二进制（Linux x86 gnu/musl + ARM64 gnu/musl、macOS ARM/Intel、Windows）
- **七产物矩阵**：Linux×4 = `ubuntu-24.04(-arm)` runner + `ubuntu:22.04` **容器**（容器锁 glibc 2.35，产物兼容性由容器基座决定与 runner 解耦——runner 退役只换 runs-on 不动产物；裸容器无 git/sudo，apt 装 deps 先于 checkout 且去 sudo）+ `macos-15`×2（ARM 原生 aarch64 + ARM 上交叉编译 x86_64；macos-14 池已被 GitHub 下线，报 `'macos-14-arm64-us-default' is not supported` 0 step 取消）+ `windows-latest`。ARM64 走原生 arm runner（避开 QEMU 交叉与 aws-lc-rs 找交叉 gcc 的坑）；musl 全静态通吃老系统/Alpine/树莓派；macOS 必须设 `MACOSX_DEPLOYMENT_TARGET=11.0`（否则 runner SDK 升高会编出老 macOS 跑不了的二进制）；Windows 保持 latest。**固定 runner label 的代价是镜像退役要手动跟进**——GitHub 在 Changelog 和 actions/runner-images 仓库提前公告 + brownout 缓冲期，不会无声挂
- **纯 CI/文档修复复用原版本号删 tag 重发，patch/minor bump 只留给代码变化**
- **本地 origin 指向 gitee，GitHub remote 叫 `github`**：查 GitHub 状态用 API 或 `git fetch github --tags`

## 提交规范

- 格式：`type(scope): description`（如 `feat(cli): add switch command`）；scope 可选，与 type 语义不同、不可同名
- 类型：feat、fix、perf、refactor、docs、test、style、build、ci、chore
- scope 仅代码模块：cli / core / util / gui；跨 crate 用逗号分隔（`feat(cli,core): ...`）；docs / ci / test / style / build 类不带 scope，发版 `chore: release vX.Y.Z`，依赖升级 `build(deps): bump ...`
- 分支命名：`feature/xxx`、`fix/xxx`、`docs/xxx`
