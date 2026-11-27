# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**sdkmate**（`sdkm`）是一款面向全栈工程师的跨平台 SDK 版本管理器，用 Rust 编写。通过符号链接切换 + 操作系统环境变量操纵，管理 Java、Node.js、Python、Maven、Go 等开发环境的版本。

## 开工前必读（文档索引）

`CLAUDE.md` 只保留稳定的项目约定；易变内容一律在 `docs/`，按任务查阅：

| 内容 | 文档 |
|:---|:---|
| 贡献流程、提交与 PR 规范 | [CONTRIBUTING.md](CONTRIBUTING.md) |
| 构建环境、质量门禁、测试沙箱、平台验证 | [docs/development.md](docs/development.md) |
| **易变开发注意事项**（历史坑、跨层契约） | [docs/development-notes.md](docs/development-notes.md) |
| 新增内置 SDK（四类、构造函数、SEEDS） | [docs/sdk-onboarding.md](docs/sdk-onboarding.md) |
| CLI 命令与参数 | [docs/commands.md](docs/commands.md) |
| config.toml / gui.toml | [docs/configuration.md](docs/configuration.md) |
| 自定义 SDK | [docs/custom-sdk.md](docs/custom-sdk.md) |
| 桌面端 | [docs/gui.md](docs/gui.md) |
| 常见问题 | [docs/faq.md](docs/faq.md) |
| 使用指南 | [docs/usage.md](docs/usage.md) |

## agent 工作边界（Fork + AI）

- 先给计划再编码；需求有歧义先问，不默默选解释。
- 只碰与任务相关的文件；一个 PR 只做一件事，不夹带重构/格式化。
- 手动验证一律 `SDKM_HOME=<临时目录>`；禁止在真实 home 执行 `init`/`switch`/`uninstall`（会写注册表/PATH/shell profile）。
- **贡献者不改**根 `Cargo.toml` 版本号、`.github/workflows/`、tag/release；发版由维护者执行。
- **本文件保持稳定**：新的开发注意事项写 `docs/development-notes.md`，不要往 `CLAUDE.md` 追加。

## 构建与开发命令

```bash
cargo build                          # 调试构建（默认只构建 CLI）
cargo build --release                # 发布构建（产出 sdkm 二进制）
cargo build -p sdkm-gui              # 单独构建 GUI
cargo test --workspace -j 2          # 全部测试；裸 cargo test 只测 default-members（CLI）
cargo test -p <crate>                # 指定 crate（sdkm-cli / sdkm-core / sdkm-util）
cargo fmt --all --check              # 格式门禁（cargo fmt 可自动修复）
cargo clippy --workspace --all-targets --all-features  # 展示不阻塞，新增代码不得增加警告
```

首次构建的原生依赖（Linux GUI 库 / macOS cmake / 运行时 ca-certificates）见 [docs/development.md](docs/development.md)。

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
  crates/sdkm-gui    → gpui-kit 桌面端：pages/（页面）、i18n/（中英文案）、data.rs（2-worker runtime 调度）；业务全部走 core
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
- **Unix 持久化 = 托管 env 文件**：PATH / extra_vars 写入 `<home>/sdkm_env.sh`（fish 为 `sdkm_env.fish`）的 `# >>> sdkm env >>>` 标记块（或 config `env_file` 指定的自定义文件）；profile 只注入 source 行 + hook 行，日常 `switch` 不碰 profile
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

`symlink_dir` 为 `Option<String>`：`None` = 跟随 home（运行时 resolve 成 `<home>/links`），`Some` = 用户自定义。exe 移到哪 links 就在哪，绿色便携。`env_file` 同为 `Option<String>`（仅 Unix）：`None` = 跟随 home（`<home>/sdkm_env.sh|fish`），`Some` = 用户自定义文件；`set`（空串=恢复默认）/`delete` 成功后自动迁移托管块内容并把 profile 的 source 行重定向到新路径。自定义文件只适用单一 shell（块内语法随写入时检测的 shell）。

### 平台抽象
- `EnvOperation` trait：`WindowsEnvOperation`（注册表 + WM_SETTINGCHANGE 广播）和 `UnixEnvOperation`（托管 env 文件读写；profile 只注入 source/hook 行）
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

用户文档（参数、示例、TUI 按键）见 [docs/commands.md](docs/commands.md)；命令 → Handler 的代码落位见 [docs/development.md · CLI 命令落位](docs/development.md#cli-命令落位)。

所有命令在 `crates/sdkm-cli/src/impls/` 中有 `CommandHandler` trait 实现，委托给 `crates/sdkm-core/src/manager/` 中的 `SdkManager` 方法。

`config` 子命令：`set`/`get`/`list`/`delete`/`edit`/`add-sdk`/`remove-sdk`，按类型校验（`ValueType`：Url/UrlTemplate/Bool/U32/Path/Token/String）+ 原子写入（写入-重命名）+ 快照回滚；内置 SDK（`SDK_SEEDS` 注册表中的全部 42 个）不可 delete/remove-sdk，只能 set 修改。

## 开发注意事项（易变）

历史坑、跨层契约与平台约束集中在 [docs/development-notes.md](docs/development-notes.md)。**新条目追加到该文件，不要改本文件**；改 install/env/switch/gui/测试前先按关键词检索。

## 提交规范

完整规范（type/scope、分支命名、PR 流程与工程约束）统一维护在 [CONTRIBUTING.md · 提交规范](CONTRIBUTING.md#提交规范)，此处不重复。agent 额外约定：

- **commit 前必须先 review 本次全部改动**：逐文件自审（逻辑/边界 bug、孤儿 import、路径写法、风格违规；Claude Code 环境可跑 `/review`）→ 处理完 review 发现的问题 → 重新编译/测试验证 → 才能提交。后台 review 未返回结果前**禁止抢跑提交**，用户说"等 review"时必须等到通知再说
- commit message 用英文、一行简短主题（不写 body；关联/关闭 issue 在 subject 末尾写 `(closes #N)`）
- 不添加 Claude 署名 / co-author 信息
- 仅当用户确认测试通过并明确要求时才 commit / push；push 前先 rebase 到远端最新，保持线性历史
- **贡献者边界**：不改根 `Cargo.toml` 版本号、不改 `.github/workflows/`、不打/删 tag、不操作 release；发版流程只在维护者本地说明，不进本文件
- PR 标题用 Conventional Commits，描述用中文讲清「改了什么 / 为什么 / 怎么验证」
