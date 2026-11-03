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

## 当前开发进度（2026-11-03）

> **维护规则**：本节只保留最新一次改动，每次完成后**整体替换**（不是追加），文件体积不随历史增长。

### 2026-11-03 —— `sdkm current` 三层生效版本解析 + 来源标注（issue #22）+ TUI tip 轮换节流

**背景**（issue #22）：多级作用域机制（session `SDKM_ACTIVE_*` > project `.sdkm.toml` pin > global `config.toml current_version`）对用户不可见——旧 `sdkm current` 只读 global 层的 `current_version`，pin/use --shell 生效时显示版本与实际生效不一致，用户无法定位"现在跑的是谁的版本"。

**实现（只增展示、零原有逻辑修改，用户硬约束）**：新增 `sdkcore/src/current.rs`——`VersionOrigin` 枚举（Session/Project(PathBuf)/Global，Display 输出 `session`/`project (路径)`/`global`）+ `EffectiveVersion{version, origin}` + `SdkManager::effective_version[_with]` 三层查询链，复用原子函数 `session_env_value`（env.rs 私有 → **pub(crate)**，唯一触碰原有文件处）/`find_project_config`/`list_local_sdk_versions`/`fuzzy_match_version_core`。**不重构 env.rs 的 `generate_env_script_inner`**（hook 热路径、脚本导向不带层信息）——current 独立薄查询链，防漂移三件套：共享原子函数 + 模块文档交叉引用声明同步义务 + 守卫测试（失效 session pin 压制 project 直落 global，锁定 env.rs:141 契约）。

**语义**：session/project 层值经 fuzzy 解析为**精确版本**后展示（与实际 PATH 生效一致）；pin 失效（未装/匹配不上）**静默降级** global（用户拍板，不加 warning——hook 路径已有 stderr 提示）；global 层 `current_version` 原样不重校验。展示：单 SDK `ℹ️  java 25.0.0 (session)`；全表三列 `sdk | current | source`，source 无值统一 `N/A`（current 列同款）。三档渐进着色（`VersionOrigin::styled()` 富文本）：session 品红斜体 > project 黄斜体（**路径段 dim 淡化**）> global 暗灰斜体。

**配套与架构**：`util/terminal.rs` 新增 `display_width`（ANSI 转义不计宽，`pad_right`/`print_table` 列宽改用——富文本单元格参与表格对齐不错位；对纯文本调用者行为不变）；`print_table` 加 `col_gap` 列间距参数（current 表用 4）。**展示/核心分层（用户架构决策）**：core 只做纯数据 API（`effective_version`/`effective_versions_all` 返回 `EffectiveVersions{entries, skipped}`，零打印零输出宏），展示逻辑全部下沉 cli 层（`CurrentHandler::run` + `styled_origin`：三档渐进富文本 session 品红斜体 > project 黄斜体路径 dim > global 暗灰斜体）——为将来 GUI 集成铺路；`show_local_sdks_current` 从 list.rs 删除对号入座。docs：use --shell 的 PowerShell 示例换 `sdkm use java 21 --shell | iex`（usage.md/commands.md 两处；**hook 的 `-join` IEX 写法不换**——hook 脚本是跨多行 prompt 函数，逐行管道执行破坏语法）。**提交规矩（新）**：文档修改（docs/*.md、CLAUDE.md）不混进 feat/fix，单独 `docs:` commit。**排查备忘**：PS 5.1 捕获 native 输出时 stderr 会被包成 ErrorRecord 混入（warning!/info! 走 stdout、error! 走 stderr），IEX 执行非脚本文本即报错——先确认命令带 `--shell`；用户终端"环境变量名或值太长"= 会话实时 PATH 超 32767（注册表 PATH 正常 ~2k），新开终端即愈。

**验证**：`tests/current.rs` 沙箱 7 用例（三层优先级/模糊精确化/双失效降级/漂移守卫/子目录向上查找）+ sdkcore 全量 98 用例全绿，clippy 零警告；临时目录冒烟 8 场景（三档单查/同表/失效降级/未注册报错不变/N/A）。未发版。

**④ TUI tips💡 区域随 size 回填疯狂闪烁（同日，`sdkm ls`）**：size 渐进渲染优化的漏网之鱼——`tip_idx` 挂在主循环**每次迭代**上，size 计算中 120ms 短轮询每轮回填都重绘，tip 也跟着 120ms 换一条；size 算完回退 3s 长轮询后自然安静（"3s 节奏"只在注释没在代码）。**修复**：tip 轮换改独立 `Instant` 时间锚点（`last_tip_at`，≥3s 才推进下一条），与重绘原因彻底解耦；两个 TUI（`run_sdk_selector_inner` + `run_selector_inner`）同型修改。用户 Windows 实测通过。

## 已知问题与注意事项

- **使用处禁止跨模块全路径，一律顶部 use 短名**（如 `util::builtin::find_seed(...)` 写体内 → 改 `use util::builtin::find_seed;`）。rustfmt 只管 use 语句、管不了使用处写法，无自动化工具；clippy `absolute_paths` 的 `max-segments=3`（为放过 `tokio::fs` 等第三方 3 段惯用写法，实测降 2 噪音比 4:1）恰好抓不到项目内 3 段路径——**review diff 时人工扫 `util::`/`crate::` 开头的体内全路径**（宏内 `$crate::` 除外，那是正确写法）。CLI help 的内置 SDK 名单固定写 "java, node, python, go, maven, ... (see docs)" 省略号形式，不随名单扩容联动修改
- Maven 有下载模板但无 `version_url`，仅支持精确版本安装（模糊版本不可用）
- Rust 完全缺失内置源配置条目
- **Windows 需管理员运行**：环境变量与 PATH 写入 `HKEY_LOCAL_MACHINE`，符号链接创建需 `SeCreateSymbolicLinkPrivilege`（管理员或开发者模式），`init`/`switch` 需管理员权限
- Unix 环境变量操作使用 `unsafe { env::set_var() }`，在 Rust 2024 edition 中属于 UB（功能生效，彻底修复需 shim 模式重构或用户手动 source）
- Python 版本解析 `per_page=100` 仅获取最近 100 个 release（仅备源 GitHub API 有限制，主源 uv metadata 无此问题）
- Rust 工具链通过 `rust-toolchain.toml` 固定为 1.92.0（edition 2024）
- **zip 仅启用 `deflate` 特性**（体积优化）：若日后解压非 deflate 压缩或密码 zip 会失败，按需在 `Cargo.toml` 加回对应 feature（`bzip2`/`lzma`/`zstd`/`deflate64`/`aes-crypto`）
- **reqwest 0.13.4 + rustls**：`default-features = false, features = ["json","gzip","default-tls"]`。`default-tls = rustls`（aws-lc-rs + rustls-platform-verifier 走系统 CA bundle），不依赖系统 OpenSSL——Linux 编译只需 `cmake`，运行时需 `ca-certificates`。禁 `http2`/`charset`/`query`/`form`
- **BugReport 标记只用于真正不可由用户修复的错误**：`install_sdk` 入口用 `?` 传播，真正的 bug（解压/校验失败、内置配置缺失）在 `install_sdk_async` 内部用 `try_bug!`/`bail_bug!` 精确标记，CLI 层 `needs_bug_report` 靠 `downcast_ref` 检测。用户取消是普通 `bail!`。switch 的 `try_step!` 不标 BugReport——步骤失败（symlink 创建 os error 1314、env 写入 os error 5）多属权限/环境问题，用户可自行解决（管理员运行），返回普通错误。**init 第 4 步 symlink_dir 不可写属用户环境问题，用 `bail!` 不用 `try_bug!`**
- **cfg(unix) 代码在 Windows 上不编译，类型错误会漏到 Linux 才暴露**：`env/unix.rs` 整个模块 `#[cfg(unix)]`。改 `unix.rs` 后**必须**验证类型——Windows 上临时把 `env/mod.rs` 的 `#[cfg(unix)] mod unix;` 改成 `mod unix;`（注释掉对应 `pub use` 避免冲突），跑 `cargo check -p sdkcore` 强制编译暴露错误，验证完改回。`cargo check --target x86_64-unknown-linux-gnu` 不行（aws-lc-rs build.rs 找不到 `x86_64-linux-gnu-gcc`）；或直接 WSL `cargo build`。**filter 闭包参数是 `&Self::Item`（item 的引用）**——`split()` 的 item 是 `&str`，闭包参数是 `&&str`，必须用 `|&p|` 解构才能跟 `&str` 比较；`&String` 用 `.as_str()` 转 `&str`。**涉及跨平台代码提交前必须保证双平台编译通过**（v0.5.1 教训：region.rs unix 分支漏 `-> bool`，tag 构建全挂，删 tag 重发补救）；单文件级验证法 = 临时剥 `#[cfg(unix)]` 属性跑 `cargo check`，完事必须恢复属性并重编译确认（sed 批量剥极易漏恢复）
- **SDKM_HOME 环境变量可覆盖 home**：`util/path.rs::get_sdkm_home` 优先读 `SDKM_HOME`，未设回退 `current_exe().parent()`，行为兼容。主要解锁 in-process 集成测试——`SdkManager::new()` 等路径全经 `get_sdkm_home`（绑 `current_exe`，原本不可注入），注入后测试可全部指向临时目录。`tests/uninstall.rs` 等用：设 `SDKM_HOME=temp` + 手工构造 `SdkManager{config, env_operation: Box<MockEnv>}`（字段 pub，`MockEnv` impl `EnvOperation` 记录调用）+ 模块级 `Mutex` 串行（`set_var` 全局竞态）+ `TestEnv` Drop 恢复 env/清理目录
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
- **根 `CHANGELOG.md` 由 CI 维护**：发版后 upload-release job 把版本正文 prepend 到根 `CHANGELOG.md` 并 commit 回 master。**本地不要手改**，会被下次发版覆盖；GITHUB_TOKEN push 不触发 workflow 递归
- **版本号只能递增、不可复用**：已发版本 tag 已存在，再 push 同版本会被跳过。重发同版本需先删 tag+release 再 push；或直接 bump 到下一版本（推荐）
- **不发 crates.io**：所有 crate `publish = false`，发布物为 GitHub Release 上七产物二进制（Linux x86 gnu/musl + ARM64 gnu/musl、macOS ARM/Intel、Windows）
- **七产物矩阵**：`ubuntu-22.04`×{gnu,musl} + `ubuntu-22.04-arm`×{gnu,musl} + `macos-14`/aarch64 + `macos-14` 交叉编译 x86_64-apple-darwin + `windows-latest`。Linux gnu 用 22.04（glibc 2.35）避开 24.04 的 `GLIBC_2.39` 墙；ARM64 走原生 arm runner（`ubuntu-22.04-arm`，避开 QEMU 交叉与 aws-lc-rs 找交叉 gcc 的坑）；musl 全静态通吃老系统/Alpine/树莓派；macOS 必须设 `MACOSX_DEPLOYMENT_TARGET=11.0`（否则 runner SDK 升高会编出老 macOS 跑不了的二进制）；Windows 保持 latest
- **纯 CI/文档修复复用原版本号删 tag 重发，patch/minor bump 只留给代码变化**
- **本地 origin 指向 gitee，GitHub remote 叫 `github`**：查 GitHub 状态用 API 或 `git fetch github --tags`

## 提交规范

- 格式：`type: description`（如 `feat: add switch command`、`fix: resolve xxx issue`）
- 类型：feat、fix、docs、refactor、test、chore
- 分支命名：`feature/xxx`、`fix/xxx`、`docs/xxx`
