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

## 当前开发进度（2026-06-30）

### 本次改动（2026-06-30）
1. **提取版本解析逻辑到公共 `version/` 模块** — 不再混在 install 里
   - 新建 `crates/sdkcore/src/version/{mod,cache,fuzzy,discovery}.rs`，从 `install/resolver.rs` 按职责迁入（逻辑不变，仅搬家 + 拆 trait）
   - 拆分 `SdkInstallStrategy` → `VersionDiscovery`（仅 `parse_version_data`，公共）；`build_download_url` 移至新建 `install/download_url.rs`（按 SDK 分发的自由函数，install 专属）
   - `get_install_strategy` → `get_version_discovery`；`ConfigBasedStrategy` → `ConfigBasedDiscovery`（单元结构体）
   - 更新引用：`lib.rs` 新增 `pub mod version`；`install/mod.rs`（discovery 改为仅在有 version_url 分支创建、`build_download_url(sdk, ...)` 替换 `strategy.build_download_url`）；`list.rs`/`switch.rs` import 改指 `crate::version`
   - 删除 `crates/sdkcore/src/install/resolver.rs`
   - 验证：clippy 告警数 22→22（与提交基线一致，**零新增**）；build/test 全绿；本次文件 fmt 干净
2. **模块重命名 + 注释中文化** — `versioning` → `version`；将 `version/`、`install/download_url.rs` 与 `list.rs` 中残留的英文注释（缓存/获取流程、各 SDK 段落标题、Step 1/2、数据结构/本地/远程列表段落等）改为中文（终端输出字符串保持英文,符合项目"终端输出英文"约定）
3. **修复模糊匹配漏匹配 `v` 前缀版本** — `sdkm s node 14` 误报 "did you mean 'v14.16.0'"
   - 根因：Node 等本地版本目录带 `v` 前缀（`v14.16.0`），而 `fuzzy_match_version_core` 未归一化——前缀匹配 `"14."` 命不中以 `v` 开头的候选；`parse_version_components("v14.16.0")` 因 `"v14"` 解析失败丢掉主版本号，导致排序/距离/建议全失真
   - 修复：新增 `strip_v_prefix`（仅当 `v`/`V` 后紧跟数字才剥离，不误伤 `version` 串）；精确/前缀匹配均基于归一化形式比较（故 `"14"` 前缀命中 `"v14.16.0"`、`"14.16.0"` 精确等于 `"v14.16.0"`）；`parse_version_components` 先剥离 `v` 再切分；返回的 `full_version` 保留**原始串**（带 `v`），switch 才能定位正确目录
   - 行为不变：精确→直接切换、模糊（单/多,取最新）→ `prompt_confirm` 确认、真正无前缀匹配（如 `node 13`）→ 仍报 "did you mean" 错误（用户确认此为期望行为）；install 共享核心同步受益
   - 新增 `fuzzy.rs` 的 `#[cfg(test)]` 模块（13 项）：v 前缀归一化、精确/前缀/多版本取最新、前缀不跨版本边界（`3.1` 不匹配 `3.10`）、无匹配 did-you-mean、空列表报错、排序与建议

### 本次改动（2026-06-29）
1. **switch 版本参数支持模糊匹配** — 与 install 共用匹配核心
   - 抽取共享核心 `fuzzy_match_version_core(versions: &[String], input) -> Result<FuzzyMatch>`（`install/resolver.rs`），与 SDK 来源无关，install/switch 复用
   - 模糊粒度到**次版本**：`"3"` → 最新 `3.x.x`、`"3.12"` → 最新 `3.12.x`（前缀方案 `input + "."`，`"3.1."` 不会误匹配 `"3.10.x"`）；顺带修正 install 原有 doc 注释已声称但未实现的 `3.12` 模糊行为
   - install 侧 `fuzzy_match_version(entries, input)` 改为薄封装：调用核心 + 回查 `VersionEntry` 填充附属字段；失败时传播核心的"did you mean"错误（install 自动获得建议提示），`install/mod.rs` 无需改动
   - switch 侧 `switch_sdk_to_version` Phase 0 接入：本地版本列表 → `fuzzy_match_version_core` → 模糊命中时 `prompt_confirm` 交互确认（与 install 一致）；后续阶段统一用 `target_version`
   - 新增 `suggest_similar_version`（最长公共数值前缀为主、数值距离为辅）+ 私有辅助 `parse_version_components`/`compare_versions_desc`/`version_distance`/`no_match_error`；匹配失败时两命令均提示 "did you mean 'X'?"

### 本次改动（2026-06-26）
1. **文档体系重建** — README + docs/ 详细用法文档
   - 重写 `README.md`（中文）/ `README-en.md`（英文）：营销向，突出"专为全栈工程师打造"定位 + 纯绿色可移植 / 即时切换 / 透明可回滚 / 可扩展 / 跨平台原生五大优势；一行命令快速上手；图标式标题（🎯/✨/📦/🚀/🎮/🏗️/🛠️/🔧/🤝/📄）保持原排版风格；单 logo 居中；修正过时信息（install/current/config 不再标"即将到来"；`list` 参数由 `--source remote` 改为 `-r/--remote`；Rust 徽章 1.80 → 1.92.0；路线图移除）
   - 新建 `docs/` 目录，拆分四篇详细文档（互相跳转）：
     - `docs/usage.md` — 详细用法总入口（快速上手 + 导航索引 + TUI + 托管已有 SDK + 已知限制附录）
     - `docs/commands.md` — 每个子命令详解（init/install/list/switch/current/config，含别名、参数、TUI 按键、示例）
     - `docs/configuration.md` — `config.toml` 结构 + 每个配置项含义（可删除性/默认值）+ 点分隔键名格式 + 8 种 ValueType 校验规则 + 原子写入/快照回滚
     - `docs/custom-sdk.md` — `add-sdk`/`remove-sdk` 用法 + URL 模板占位符系统（`{version}`/`{os}`/`{arch}`/`{ext}`/`{feature_version}`/`{release_tag}`/`{platform}`/`{sdk_dir}` 等）+ 内置 SDK 源参考
   - 已知限制从 README 移至 `docs/usage.md` 末尾附录；docs 中不提及 Rust 内置源缺失与 CLAUDE.md

### 本次改动（2026-06-23）
1. **config 命令** — 完整实现 config 命令系统（set/get/list/delete/edit/add-sdk/remove-sdk）
   - 新增 `ConfigKey`/`SdkField`/`ValueType`/`ValidatedValue`/`KeyMeta` 类型定义（`sdkcore/manager/config.rs`）
   - 按类型校验：`validate_by_type()` 根据 `ValueType`（Url/UrlTemplate/Bool/U32/Path/Token/String）校验值格式
   - 键名解析：`parse_config_key()` 将点分隔字符串映射到具体配置路径
   - 内置 SDK 保护：所有字段不可 delete，不可 remove-sdk，只能 set 修改
   - 原子写入：`atomic_write_to_disk()` 替代 `fs::write()`（写入-重命名模式）
   - 快照回滚：`ConfigSnapshot` + `rollback_config()`/`rollback_config_from_raw()`，set/delete/add-sdk/remove-sdk 失败自动恢复
   - `write_to_disk()` 内部改为调用 `atomic_write_to_disk()`（向后兼容）
   - CLI 层 `ConfigHandler` + 7 个子命令 handler（`cli/impls/config.rs`）
   - `edit` 子命令：检测 `$EDITOR/$VISUAL` → 平台 fallback（Windows: notepad, Unix: vi）→ 调用外部编辑器 → TOML 校验
   - `add-sdk` 子命令：参数式创建自定义 SDK（必填 download_url + bin_dir，可选版本源 URL + extra_vars/extra_paths）
   - `remove-sdk` 子命令：移除自定义 SDK（内置 SDK 不可移除）
   - 新增 `regex-lite` workspace 依赖（URL 模板占位符替换）
   - `lib.rs` 中 `Commands::Config` 从裸变体改为带 `ConfigHandler` 参数

### 本次改动（2026-06-22）
1. **switch 备份回滚机制** — `switch_sdk_to_version` 重构为 Snapshot + 显式回滚链模式
   - 定义 `SwitchSnapshot` 结构体：备份旧符号链接目标、旧环境变量值、已添加 PATH 条目、旧 config 快照
   - 新增 `EnvOperation` trait 方法：`get_env_value`（读旧值）、`unset_sdk_env`（删变量）、`restore_sdk_envs`（批量恢复）
   - Windows 实现：注册表读取旧值 + 删除注册表值 + 批量写回旧值
   - Unix 实现：shell profile 读取旧值 + 删除 export 行 + 批量恢复
   - `link/symlink.rs` 新增 `read_symlink_target`（备份旧链接目标）+ `remove_symlink`（回滚删除）
   - `try_step!` 宏：消除 5 处重复的 `if-let-Err + rollback` 模式，自动嵌入 `BugReport` 标记
   - `RollbackFailure` 结构体：只输出对用户有影响的结果 + 修复建议，不暴露内部细节
2. **Bug Report 链路** — 命令失败时自动提示 GitHub issue
   - `util/consts.rs` 新增 `BugReportError` 包装器（类型安全的 downcast 检测，不修改用户可见的错误消息）+ `GITHUB_ISSUES_URL` 常量
   - `util/macros.rs` 新增 `try_bug!`（自动 BugReportError::wrap）和 `bail_bug!`（bail + BugReportError）两个宏
   - `util/terminal.rs` 新增 `suggest_bug_report()` 公共函数
   - `cli/lib.rs` 全局错误处理重构：`command_name()` + `needs_bug_report()` + `suggest_bug_report` + `process::exit(1)`
   - 白名单模式：只在源头标记不可由用户解决的错误，其余用户错误不触发 bug report
   - 已标记的子命令错误点：
     - **switch**: try_step! 宏自动标记（符号链接、环境变量、PATH、config 写入失败）
     - **install**: tokio runtime 创建、临时目录创建、解压、目录调整、安装验证失败
     - **init**: 目录创建、PATH 添加、配置写入、符号链接目录创建失败
     - **tui**: 终端 raw mode、alternate screen、事件轮询/读取、终端恢复失败
     - **list**: tokio runtime 创建、内置配置缺失失败

### 已完成并提交的改动（2026-06-17）
1. **依赖更新** (`2898678`) — 新增 tokio、indicatif、zip、flate2、tar、futures-util、serde_json；reqwest stream feature
2. **util 层增强** (`428362d`) — 终端输出重构（统一调色板 + detail/step/divider 宏）、模板渲染升级（OsStyle/ArchStyle + 新占位符）、SDK 类型扩展（primary_executables、Maven 配置）
3. **env 模块扩展** (`75d1474`) — EnvOperation 新增 remove_sdk_path、path_separator/split_path_entries 公共辅助、Windows/Unix 重构
4. **switch 增强** (`9c61aec`) — PATH 冲突检测 + extra_paths + github_token 配置字段
5. **install 模块拆分** (`2d72e73`) — 单文件拆为 5 子模块（mod/resolver/downloader/extractor/progress），12 阶段异步安装流程
6. **CLAUDE.md** (`18c902d`) — 项目架构文档 + 进度追踪

### 本次改动（2026-06-18，待提交）
1. **CLI 参数简化** — `--source` 改为 `-r/--remote` 布尔 flag；新增 `--limit` 参数（默认 20）；报错信息改为英文
2. **交互式 TUI 版本选择器** — 新建 `cli/tui.rs`：crossterm raw mode + alternate screen；↑↓/jk 导航；i 触发安装（远程）；Enter/s 触发切换（已安装）；Ctrl+C/q/Esc 退出；旋转 tips；远程源 URL 顶部透明展示；总版本数+显示数量在 title 行
3. **远程版本列表** — 复用 resolver `fetch_version_data + parse_version_data` 管道；spinner 加载进度；安装状态标记（✅ active / 📦 installed / blank = not installed）；`source_url` 字段透明展示；`RemoteVersionResult` 包含 total_count
4. **缓存优先策略** — `resolver.rs` 从"缓存兜底"改为"缓存优先 + TTL"：TTL 值从硬编码常量改为 `NetworkConfig.cache_ttl_secs` 配置项（默认 3600秒）；基于文件 mtime；过期后才走 API；API 失败时退化返回 stale cache
5. **本地 SDK 列表增强** — `sdkm list` 显示所有 SDK + 当前版本
6. **英文输出** — 所有终端输出改为英文
7. **常量统一** — `DIVIDER`、`STATUS_ACTIVE`、`STATUS_INSTALLED`、`TUI_TIPS` 集中在 `util/consts.rs`；`terminal::divider()` 使用常量而非 `"─".repeat(50)`
8. **TUI 体验优化** — unicode_width 对齐 emoji 列；三级颜色（选中=白+DarkCyan bg / 已安装=绿 / 未安装=灰）；固定 MAX_VISIBLE=10 视口递补滚动；Ctrl+C 退出支持；keybinding 标注 (installed)/(remote)
9. **init 命令增强** — 目录部署检测（`DeploymentAssessment`：Good/Suspicious，只检查末尾路径组件是否含 "sdkm"）；初始化流程透明展示 4 步 + 每个目录用途说明；`suggest_sdkm_path()` 平台建议路径；`--force` 时跳过目录检测；注释改为中文

### list 子命令新格式
| 命令 | 行为 |
|---|---|
| `sdkm list` | 非交互：打印所有已安装 SDK + 当前版本 |
| `sdkm list java` | 交互式 TUI：本地版本选择器，按 s 切换版本 |
| `sdkm list java -r` | spinner 加载 → 交互式 TUI：远程版本选择器，按 i 安装、s 切换 |
| `sdkm list -r` | 报错："Please specify an SDK name" |
| `sdkm list maven -r` | 报错："Maven does not support remote version listing" |

## 已知问题与注意事项

- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- Windows 环境变量操作写入 `HKEY_LOCAL_MACHINE`（需要管理员权限），非 `HKEY_CURRENT_USER`
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB
- 现有集成测试使用硬编码的 Windows 绝对路径——不可移植，无单元测试（`#[cfg(test)]` 模块）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有此限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.92.0（edition 2024）

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

## 提交规范

- 格式：`type: description`（如 `feat: add switch command`、`fix: resolve xxx issue`）
- 类型：feat、fix、docs、refactor、test、chore
- 分支命名：`feature/xxx`、`fix/xxx`、`docs/xxx`
