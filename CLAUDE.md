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

**sdkcore 内部模块**：`version` 是版本解析公共模块(install/switch/list 共用),含 `cache`(缓存+fetch)、`fuzzy`(模糊匹配+相近版本建议)、`discovery`(各 SDK 版本发现+resolve 编排);下载 URL 构建留在 `install/download_url`(install 专属)。

**依赖关系**：`cli → sdkcore, util`；`sdkcore → util`；`util` 无内部依赖。所有 crate 继承 workspace 元数据（版本、edition、license），均为 `publish = false`。

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
download_url = "..."
bin_dir = "bin"
```

### 平台抽象
- `EnvOperation` trait：`WindowsEnvOperation`（注册表 + WM_SETTINGCHANGE 广播）和 `UnixEnvOperation`（shell profile 修改）
- `cfg(windows)` / `cfg(unix)` 条件编译用于环境操作、符号链接、默认路径
- 类型别名 `OsEnvOperation` 在编译时选择平台实现

### Home 目录发现
- sdkm 的 "home" = 运行中可执行文件的父目录（`current_exe()`），使工具可移植
- Store 目录：`<exe_parent>/store/`，用户在此放置 SDK 安装（如 `store/java/21/`）
- 配置文件：`<exe_parent>/config.toml`，TOML 格式，serde 序列化/反序列化 + 模板渲染

### 模板渲染系统
- `TemplateRenderer` 解析 URL 和环境变量配置中的占位符：`{sdk_dir}`、`{sdkm_home}`、`{sdks_install_dir}`、`{os}`、`{arch}`、`{ext}`
- OS/arch 检测使用 `OnceLock` 静态变量；映射 macOS→darwin、x86_64→x64、aarch64→arm64

### 错误处理与输出
- 全项目使用 `anyhow::Result`（启用 backtrace 特性）；`anyhow::bail!` 和 `.context()` 处理错误
- 无自定义错误类型，不使用 thiserror（`BugReport` 标记类型除外——用于 CLI 层检测不可由用户解决的错误）
- 终端输出通过自定义宏：`info!`、`success!`、`warning!`、`error!`、`detail!`（crossterm 彩色，不是 `log` crate）
- CLI 错误：用 `error!` 宏打印；debug 构建额外显示 backtrace；检测 `BugReport` 标记时提示 GitHub issue URL
- CLI 错误退出码：失败时 `process::exit(1)`

## CLI 命令结构

| 命令 | 别名 | Handler | 状态 |
|------|------|---------|------|
| `sdkm init` | — | cli/InitHandler | 已实现（目录部署检测 + 项目目录识别 + 平台建议路径） |
| `sdkm install <SDK> <VERSION>` | `i` | cli/InstallHandler | 已实现（模块拆分为 download_url/downloader/extractor/progress，版本解析提取到 `version/` 公共模块，12阶段异步流程） |
| `sdkm list [SDK] [--remote] [--limit N]` | `ls`, `l` | cli/ListHandler | 已实现（交互式 TUI 选择器 + 远程版本 + 安装/切换动作触发） |
| `sdkm switch <SDK> <VERSION>` | `s` | cli/SwitchHandler | 已实现（PATH 冲突检测 + extra_paths 支持 + **备份回滚机制** + **版本模糊匹配（与 install 共用核心）**） |
| `sdkm current [SDK]` | `c` | cli/CurrentHandler | 已实现 |
| `sdkm config` | — | cli/ConfigHandler | 已实现（set/get/list/delete/edit/add-sdk/remove-sdk 子命令，按类型校验 + 原子写入 + 回滚） |

每个命令在 `crates/cli/src/impls/` 中有 `CommandHandler` trait 实现，委托给 `crates/sdkcore/src/manager/` 中的 `SdkManager` 方法。

### install 子模块架构（重构后）

原 `install.rs` 单文件已拆为 `install/` 模块目录；版本解析逻辑已进一步提取到公共模块 `version/`(见下)：

- `mod.rs` — 安装流程入口，12 阶段同步/异步编排（resolve → check local → build URL → download → extract → verify → normalize → verify install → cleanup → auto-switch）
- `download_url.rs` — 下载 URL 构建：按 SDK 分发的自由函数 `build_download_url`（各 SDK os/arch 风格集中在此；install 专属，不属于版本解析）
- `downloader.rs` — 下载：reqwest 客户端构建 + 主/备源切换 + 重试机制
- `extractor.rs` — 解压：tar.gz/zip 解压 + 目录标准化 + 安装验证
- `progress.rs` — 进度显示：各阶段 indicaotr 风格的进度条

### version 公共模块（2026-06-30 提取）

版本解析逻辑从 `install/resolver.rs` 提取为公共模块 `crates/sdkcore/src/version/`（install/switch/list 三方共用），按职责拆为多子文件：

- `mod.rs` — 模块声明 + 公共 re-export + `truncate` 辅助
- `cache.rs` — 版本数据缓存 + 网络获取：`VersionSource`、`fetch_version_data`（主备切换 + 重试 + 缓存兜底）
- `fuzzy.rs` — 纯版本字符串模糊匹配：`FuzzyMatch`、`fuzzy_match_version_core`、`suggest_similar_version`（最长公共前缀 + 数值距离）
- `discovery.rs` — 各 SDK 版本发现 + 解析编排：`VersionEntry`/`ResolvedVersion`/`VersionDiscovery` trait（仅 `parse_version_data`）/ `get_version_discovery` / `fuzzy_match_version`（薄封装）/ `resolve_sdk_version` / `resolve_java_version` + 各 SDK 发现结构体 + serde 解析

**拆分要点**：原 `SdkInstallStrategy` trait 拆为 `VersionDiscovery`（只含 parse，公共）+ install 侧自由函数 `build_download_url`（按 SDK 分发，各 SDK os/arch 风格集中）；`get_install_strategy` → `get_version_discovery`；`ConfigBasedStrategy`（带 os_style/arch_style 字段）→ `ConfigBasedDiscovery`（单元结构体，custom SDK 下载 URL 风格改由 `build_download_url` 的 Custom 分支用 `OsStyle::Default`/`ArchStyle::Default` 表达，与原 `ConfigBasedStrategy::default()` 行为一致）。

## 当前开发进度（2026-07-07）

### 本次改动（2026-07-07）—— Linux 可用性修复 + reqwest 升级 rustls

WSL/Ubuntu 22.04 实测暴露的 Linux 下 init/switch 缺陷集中修复，核心是**让 symlink_dir 跟随 sdkm home**（消除权限墙、跨平台一致、绿色便携）。

1. **reqwest 0.12.7 → 0.13.4（default-tls 改 rustls）** — 根 `Cargo.toml:24` 仅改 version + 注释，features 仍 `["json","gzip","default-tls"]`。0.13 起 `default-tls = rustls`（aws-lc-rs + rustls-platform-verifier），**消除系统 OpenSSL 依赖**：Linux 编译只需 `cmake`（aws-lc-rs 编 C），运行时需 `ca-certificates`。sdkmate 用到的 API 在 0.13 全稳定，零 breaking（`query`/`form` 变默认禁用 feature 但本项目没用）。体积影响待实测（aws-lc-rs 静态链入，对比旧 2.74MB）
2. **symlink_dir 重设计为 `Option<String>`**（`crates/util/src/consts.rs` + `path.rs` + `config/mod.rs`）— 删 `SDKM_SYMLINK_DIR` 平台绝对路径常量（Win `C:\Program Files\sdkm` / Unix `/usr/local/sdkm`，普通用户不可写）和死字段 `home_dir`（无人读、语义会过期）。`symlink_dir: Option<String>` + `#[serde(skip_serializing_if = "Option::is_none")]`：**None = 跟随 home**（运行时 `resolved_symlink_dir()` resolve 成 `<home>/links`），**Some = 用户自定义**。`switch.rs`/`init.rs` 改用 `resolved_symlink_dir()`；`config get/list` 显示 resolved 实际生效路径；`config set` 空值→None、非空→Some；`config delete symlink_dir` 恢复 None（deletable 改 true）；`validation.rs` 的 `default_desc` 用动态 `get_default_symlink_dir()`。exe 移到哪 links 就在哪，真正绿色便携
3. **init 流程重构**（`crates/sdkcore/src/init.rs`）— 三处改动：
   - **非 force 模式不覆盖现有 config**：config.toml 已存在时只补建缺失目录，不再 `return` 跳过。修复「改了 symlink_dir 后重跑 init 仍报 already initialized、symlink 不补建」的死循环
   - **force 模式才写默认 config**：用户明确重置时覆盖，其余情况保留用户配置
   - **第 4 步 symlink_dir 不可写时 `bail!` 给清晰提示**：指向 `sdkm config set symlink_dir <可写目录>`，不报裸 `Permission denied`。属用户环境问题用 `bail!` 不用 `try_bug!`
4. **bug #2：PATH 新建行追加 `:$PATH`**（`crates/sdkcore/src/env/unix.rs::add_path_entry`）— `.bashrc` 无原有 `export PATH` 行时，新建行从 `export PATH="<dir>"` 改为 `export PATH="<dir>":$PATH`。原写法 source 后会把 PATH 冲成只有 sdkm 目录，`ls`/`grep` 等系统命令全废
5. **bug #4：node bin_dir 平台分支**（`crates/util/src/sdk.rs::get_sdk_bin_dir`）— Node 从全平台 `""`（根目录）改为 `if cfg!(target_os="windows") { "" } else { "bin" }`，与 Python 同款。Linux/macOS 上 node tar.gz 解压后 `node`/`npm` 在 `bin/` 子目录，原配置导致 switch 加 PATH 指向根目录、`node` 命令找不到
6. **unix.rs 重构**（`crates/sdkcore/src/env/unix.rs`）— 提常量（`PATH_SEPARATOR`/`PROFILE_BASHRC`/`PROFILE_ZSHRC`/`EXPORT_PREFIX`/`PATH_EXPORT_PREFIX`/`PATH_BACKREF`）+ 抽 helper（`read_profile`/`write_profile`/`find_path_export_line`），消除 5+ 处读/写/找行重复。清掉 unused import（`BufReader`/`OpenOptions`/`Write`）；`add_path_entry` 两层冗余存在检查简化成单层条目检查；`unsafe set_var` 加注释说明为何不彻底修。行为不变
7. **init tree 显示 symlink**（`crates/sdkcore/src/init.rs` + `consts.rs::DIR_DESC_LINKS`）— 初始化成功时的目录树加 `links/` 行：在 home 下用相对名，自定义到 home 外用绝对路径 + `(custom)` 标注
8. **文档同步**（`docs/configuration.md` + `skills/SKILL.md`）— 删旧 symlink_dir 默认值（`C:\Program Files\sdkm` / `/usr/local/sdkm`），改成"默认 `<home>/links` 跟随 home、`delete` 可恢复、`set` 可自定义"；`configuration.md` 的 `bin_dir` 说明补充 Node/Python 平台预设；`ssl_verify` 补 rustls 走系统 CA bundle

未改的技术债：`env/unix.rs` `unsafe { env::set_var()`（2024 edition UB，功能生效，彻底修复需 shim 模式重构）；`tests/env.rs` 硬编码 Windows 路径已有 `#[cfg(windows)]` 守卫不污染 Linux。

### 历史改动（2026-07-02）

### 本次改动（2026-07-02）
1. **发布二进制体积优化**（6.34 MB → 2.74 MB，降 56%）——三管齐下，零功能影响：
   - **zip 裁剪默认特性**：`zip = { version = "2", default-features = false, features = ["deflate"] }`。默认特性会拉入 `zstd-sys`/`lzma-sys`/`bzip2-sys`/`aes`+`hmac`+`pbkdf2`+`sha1` 加密套件等 C 库死重量；sdkm 解压的 SDK zip 均为标准 deflate+无密码，只留 `deflate` 足够。**最大头**
   - **reqwest 裁剪默认特性**：`default-features = false, features = ["json","gzip","stream","default-tls"]`。去掉默认的 `http2`（省 `h2`+`tokio-util`+`tracing`+`indexmap`+`hashbrown`）和 `charset`（省 `encoding_rs`）；`default-tls` 在 Windows 用系统 schannel 不增体积；sdkm 全程 HTTP/1.1 下载。顺带移除未使用的 `blocking`/`multipart` feature
   - **`[profile.release]` 体积优化**：`opt-level=3`（保持性能）+ `lto=true`（跨 crate 去重）+ `codegen-units=1`（最大化 LTO）+ `panic="abort"`（去 unwind landing pads，项目无 catch_unwind/Drop 终端守卫故无影响）+ `strip="symbols"`（剥符号，PDB 独立保留）
2. **移除 futures-util 直接依赖** — downloader 用 `resp.chunk().await`（reqwest 原生方法，返回 `Option<Bytes>`）替代 `bytes_stream()` + `StreamExt::next()`，顺带去掉 reqwest 的 `stream` feature。代码更简洁；体积无变化（futures-util 仍被 hyper-util/tower 传递依赖拉入，但不再作为直接依赖声明）
3. **GitHub Actions 发布流程** — 弃用 release-plz，改自定义 tag+release 链式工作流（`.github/workflows/release.yml`）
   - 不发 crates.io（所有 crate `publish = false`）；发布 = GitHub Release 附跨平台二进制（linux/macOS/windows）
   - 单 run 链式：master push → `tag` job 按版本号打 tag（输出 `tag_created`/`version`）→ `build` job（`if: tag_created=='true'`）三平台构建 → `upload-release` 发版。原因：默认 `GITHUB_TOKEN` 推 tag 不触发其他 workflow run（GitHub 防递归），故不依赖 tag-push 事件
   - bump 触发：只改根 `Cargo.toml` 的 `[workspace.package] version` 一行 + push 即发版（子 crate `version.workspace = true` 继承；内部 path 依赖 path-only 无 version 约束）。版本号不变则 tag job 跳过 build/release——**不是每次提交都发版**
   - 详见下方「发布流程」章节

### 本次改动（2026-07-03）
1. **下载写盘加 128KB BufWriter** — `download_with_progress` 用 `tokio::io::BufWriter::with_capacity(128*1024, file)` 包裹文件句柄，攒满再 flush，减少写盘系统调用次数（reqwest chunk 通常 8-16KB，原本每块一次 syscall）。零依赖、零风险，对大文件下载有边际收益。去 stream feature 不影响流式（`chunk()` 是 `Response` 基础方法，不依赖 stream feature）
2. **downloader 集成测试** — `crates/sdkcore/tests/install.rs`，用 `std::net::TcpListener` 起本地 HTTP server（零外部依赖），验证 `download_with_progress` 在小 body（1B）与 >128KB body（触发 BufWriter 多次 flush 的边界）下文件内容完整。按 Rust 规范：集成测试（测公共 API、起 server 端到端）放 `tests/`，单元测试（测私有函数如 fuzzy）留源码内 `#[cfg(test)]`
3. **整理 sdkcore `tests/` 目录** — 按模块名重命名（保留 git 历史）：`test_toml.rs→config.rs`、`test_env.rs→env.rs`、`test_symlink.rs→link.rs`、新增 `install.rs`。每个 `.rs` 是独立测试 binary（Rust 默认）。集成测试所需 reqwest/tokio/indicatif 声明在 `sdkcore/Cargo.toml [dev-dependencies]`（dev-only，不进发布二进制）
4. **体积优化后二进制 ~3MB** — 历经 zip/reqwest feature 裁剪 + release profile（LTO/codegen-units=1/panic=abort/strip）后，release 二进制 6.34MB→2.74MB（降 56%）。README 核心优势更新为四格（纯绿色轻量 / 即时切换全局生效 / 透明可回滚 / AI Agent 友好），~3MB 用 `<strong>` 加亮；同步 `README-en.md`
5. **CI 发版后追加根 `CHANGELOG.md`** — `release.yml` 的 upload-release job 在 softprops 发版后，把本次版本正文（去掉 gen_changelog 的 `## 🚀 What Changed` 标题）包上 `## v{VERSION} - {DATE}` 版本头，prepend 到根 `CHANGELOG.md`（保留 `# Changelog` 标题，最新版本在上），commit + push 回 master。GITHUB_TOKEN push 不触发 workflow 递归（与 tag push 同理）。此前 `1a77a6f` 删过 release-please 残留 CHANGELOG.md，现重新由 CI 维护
6. **changelog 自动生成** — `.github/scripts/gen_changelog.sh` 解析「上个 tag..HEAD」的 conventional commits，按 feat/fix/refactor/docs/ci/... 分组（emoji 标题 + per-commit 链接），经 `body_path` 写入 release body。upload-release 前置 `gh release delete` 清旧 release（softprops 不覆盖已存在 release 的 body，只重传 assets）
7. **ExitCode 重构** (`7932c89`) — `main()` / `cli.run()` 直接返回 `std::process::ExitCode`（不再 i32 + `as u8`）；agent 可凭退出码判断 sdkm 操作结果（0 成功 / 1 失败）
8. **skills/SKILL.md** (`7f28532`) — 给 Claude Code/Codex 等 agent 参考的 sdkm 使用说明 skill（自包含，含退出码判断章节）
9. **unix PATH 过滤器类型修复** (`28b2c43`) — `env/unix.rs` PATH 移除过滤器改 `|&p|` 解构，修 `&&str` vs `String` 类型不匹配（cfg(unix)，Windows 上编译不到所以之前没暴露）
10. **清理 stale CHANGELOG.md** (`1a77a6f`) — 删除根 + 三 crate 的 release-please 残留 CHANGELOG.md（release body 现由脚本现生成，不再维护仓库内 changelog 文件）。注：2026-07-03 起 CI 重新维护根 `CHANGELOG.md`（见上条 5）

## 已知问题与注意事项

- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- Windows 环境变量操作写入 `HKEY_LOCAL_MACHINE`（需要管理员权限），非 `HKEY_CURRENT_USER`
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB（`source_profile` 起子 shell source 后写回当前进程 PATH，功能上生效但有 UB 风险；彻底修复需重构为 shim 模式或让用户手动 source）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有此限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.92.0（edition 2024）
- **zip 仅启用 `deflate` 特性**（体积优化）：若日后解压非 deflate 压缩（bzip2/lzma/zstd/deflate64）或密码保护的 zip 会失败，按需在 `Cargo.toml` 加回对应 feature（`bzip2`/`lzma`/`zstd`/`deflate64`/`aes-crypto`）
- **reqwest 0.13.4 + rustls**（2026-07-07 升级）：`default-features = false, features = ["json","gzip","default-tls"]`。0.13 起 `default-tls = rustls`（aws-lc-rs 作 crypto provider + rustls-platform-verifier 走系统 CA bundle），**不再依赖系统 OpenSSL**——Linux 编译只需 `cmake`（aws-lc-rs 编 C），运行时需 `ca-certificates`。仍禁 `http2`/`charset`/`query`/`form`（sdkm 全程 HTTP/1.1 + JSON ASCII + token 走 `default_headers` 不用 form）。aws-lc-rs 静态链入，release 体积待实测（对比旧 2.74MB）
- **BugReport 标记只用于真正不可由用户修复的错误**：`install_sdk` 入口不要用 `try_bug!` 整体包裹（曾导致用户取消 `bail!("Installation cancelled by user")`、网络/版本解析失败被误报为 bug，触发 "This might be a bug in sdkm" 提示）。入口用 `?` 传播，真正的 bug（解压/校验失败、内置配置缺失等）已在 `install_sdk_async` 内部用 `try_bug!`/`bail_bug!` 精确标记 `BugReportError`，CLI 层 `needs_bug_report` 靠 `downcast_ref` 检测。switch 同理：`try_step!` 只在回滚后的中间步骤失败时标 BugReport，用户取消是普通 `bail!`。**init 第 4 步 symlink_dir 不可写属用户环境问题，用 `bail!` 不用 `try_bug!`**

### config 命令架构

| 子命令 | 功能 | 键名格式 |
|--------|------|----------|
| `sdkm config set <KEY> <VALUE>` | 设置配置值（按类型校验后写入） | `network.proxy`, `sdk.java.download_url` |
| `sdkm config get <KEY>` | 获取配置值（敏感值自动脱敏） | 同上 |
| `sdkm config list` | 列出所有配置项 | — |
| `sdkm config delete <KEY>` | 删除配置值（恢复为默认值，内置 SDK 不可删除） | 同上 |
| `sdkm config edit` | 用系统编辑器打开配置文件 + TOML 校验 | — |
| `sdkm config add-sdk <NAME> <ARGS>` | 新增自定义 SDK 条目 | — |
| `sdkm config remove-sdk <NAME>` | 移除 SDK 条目（内置 SDK 不可移除） | — |

**设计特点**：
- **按类型校验**：校验逻辑绑定在 `ValueType` 上（Url/UrlTemplate/Bool/U32/Path/Token/String），新增字段只需声明类型自动获得校验
- **原子写入**：`atomic_write_to_disk()` 使用写入-重命名模式，替代 `fs::write()` 直接写入
- **快照回滚**：set/delete/add-sdk/remove-sdk 操作失败时自动回滚（`ConfigSnapshot` + 内存级恢复 + 磁盘级原始内容恢复）
- **内置 SDK 保护**：内置 SDK（java/node/python/maven）不可 delete 任何字段，不可 remove-sdk，只能通过 set 修改

## 发布流程

工作流 `.github/workflows/release.yml` 在每次 push 到 master 时跑 tag job，但**只有版本号 bump 才会真正发版**——版本号对应 tag 已存在则跳过 build/release。所以不是每次提交都发版。

### 发版三步
1. 改根 `Cargo.toml` 的 `[workspace.package] version` 一行（patch `0.2.0→0.2.1` / minor `→0.3.0` / major `→1.0.0`）
2. 本地 `cargo build` 刷新 `Cargo.lock`（CI 用 `--locked`，Cargo.lock 必须与版本号同步，否则构建失败）
3. 单独 commit bump（`chore: release vX.Y.Z`）+ push 到 master → 工作流自动打 tag、三平台构建、创建 release

### 要点
- **changelog 自动生成**：来自「上个 tag..HEAD」的 conventional commits，按 `feat`/`fix`/`refactor`/`docs`/`ci` 等前缀分类（`.github/scripts/gen_changelog.sh`）。保持约定式 commit message 才好看
- **根 `CHANGELOG.md` 由 CI 维护**：发版后 upload-release job 把本次版本正文 prepend 到根 `CHANGELOG.md`（`## v{VERSION} - {DATE}` 版本头，最新在上）并 commit 回 master。**本地不要手改**——会被下次发版覆盖；GITHUB_TOKEN push 不触发 workflow 递归
- **版本号只能递增、不可复用**：已发版本 tag 已存在，再 push 同版本会被跳过
- **重发同一版本**：先删对应 tag+release 再 push；或直接 bump 到下一版本（推荐，更干净）
- **不发 crates.io**：所有 crate `publish = false`，发布物为 GitHub Release 上的三平台二进制
- **本地 origin 指向 gitee，GitHub remote 叫 `github`**：查 GitHub 状态用 API 或 `git fetch github --tags`

## 提交规范

- 格式：`type: description`（如 `feat: add switch command`、`fix: resolve xxx issue`）
- 类型：feat、fix、docs、refactor、test、chore
- 分支命名：`feature/xxx`、`fix/xxx`、`docs/xxx`
