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

## 当前开发进度（2026-08-28）

### 本次改动 —— 修 hook 缓存打破「会话 > 项目」优先级（会话指纹判据）

用户实测暴露：项目 pin（`sdkm use node 16` 写 `.sdkm.toml`）后跑 `sdkm use --shell node 25`（父 shell 设 `SDKM_ACTIVE_NODE`），同目录下 `node -v` 仍是项目版本，`cd` 出去才变 v25——与文档「临时 > 项目 > 全局」矛盾。定位：**不是解析逻辑错**（`shell/env.rs` 三层解析本就 session 先查、project 遇 session 已覆盖直接跳过），而是 **hook 缓存 key 漏了会话状态**：`hook_cache.rs` 命中判据只有 PWD + `.sdkm.toml` mtime + shell + schema，`use --shell` 只在父 shell 设环境变量、不碰缓存 → 下一条 prompt hook 命中旧缓存吐项目版本脚本。同 PWD 缓存 miss 的时机（首次 cd 进 / mtime 变）反而"碰巧"正确，故用户看到"项目目录里 shell 覆盖不生效、离开目录才生效"。

**修法（指纹比对）**：`HookEntry` 增 `session_fingerprint` 字段（当前进程所有 `SDKM_ACTIVE_*` 变量按名排序拼 `name=value;` 连接，空集=空串；`current_session_fingerprint()` 公开函数，env.rs put 时采样、测试直接引用）。`resolve` 增判据：条目指纹 ≠ 当前指纹 → miss 重算。覆盖 set/改/unset 全路径，不依赖各 setter 记得失效缓存。**schema 3→4 bump 必要**：旧条目 `serde(default)` 指纹为空串，与"当前无会话变量"指纹相同，不 bump 会误命中 bug 时期旧条目。

**回归测试**：`env_script_recomputes_when_session_var_set`（shell_integration.rs）——先缓存项目 pin(21) 脚本 → 设 `SDKM_ACTIVE_JAVA=25` → 断言重算出 25 bins 且无 21（复现用户 bug 场景）；Drop 守卫 + 前置 remove_var 防真实会话变量污染测试。既有 `hook_cache_cross_shell_miss`/`hook_cache_schema_mismatch_miss` 的 HookEntry 字面量补指纹字段（跟随当前指纹 = 模拟同状态命中）。

**验证**：workspace 全量 59 passed 无回归（sdkcore lib 23 + shell_integration 8 + 其余）；clippy 本次改动文件零警告（既有警告非本次引入）。已 build release 并覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（备份 .bak），用户可直接复测原日志场景。**未发版**。

**改动文件**（3）：`crates/sdkcore/src/hook_cache.rs`（指纹函数+判据+schema bump）、`crates/sdkcore/src/shell/env.rs`（put 采样指纹）、`crates/sdkcore/tests/shell_integration.rs`（回归测试+字面量补字段）。

### 上次改动 —— 修 `env/unix.rs` 两个 PATH 持久化 bug（2026-08-27）

承接上次（文档补全 v0.4.0），本次代码改动（WSL 用户排查"项目级版本切换重启后失效"时定位出的两个 bug）：

**Bug 1（dedup 回退进程 `$PATH`，silent footgun）**：`add_path_entry`（unix.rs）用 `get_path_from_content` 做"已存在则跳过"，该函数在 `.bashrc` 无 `export PATH=` 行时**回退到进程当前 `$PATH`**。后果：目录已在 live `$PATH` 但未持久化时（典型：`self uninstall` 删了 profile 行、同会话 live `$PATH` 仍残留 → 重跑 `init`）误判"已存在"跳过写入 → 重启丢 PATH。修：dedup 只查 profile 自己的 `export PATH=` 行（`find_path_export_line` + `parse`），不读进程 env；删因之孤立的 `get_path_from_content`。`add_sdk_path` 被 `init` + `switch` 共用，故 switch 的 PATH 持久化同步受益。

**Bug 2（重跑 init 时 PATH 行落到 hook 行之后）**：`add_path_entry` 无 `export PATH=` 行时走 `lines.push` 追加末尾。若 hook 行已存在（重跑 init 常见），PATH 行落在 hook 行之后 → source 时先跑 `eval "$(sdkm hook bash)"`（此时 sdkm 不在线）→ `PROMPT_COMMAND` 注册失败 → 项目 pin 不生效。修：新建 PATH 行插到 hook marker 行之前（新增 `find_hook_marker_line`，marker 取 `shell.syntax().inject_marker`），无 marker 则追加末尾。首次 init 不受影响（step2 在 step5 之前，顺序本就正确）。

**顺手修**：`write_profile` 加尾换行（POSIX；既有的 `bash_rebuild_line_add_path_entry` 测试期望带 `\n` 却因 CI 不跑 lib 测试一直静默失败，一并修好）。

**回归测试**（`env/unix.rs` tests 模块，`#[cfg(all(test, unix))]`）：新增 `add_path_entry_writes_even_if_dir_in_process_path`（Bug 1 守护，设进程 `$PATH` 含目标目录 + 空 profile，断言仍写入）+ `add_path_entry_inserts_path_before_hook_line`（Bug 2 守护，断言 PATH 行索引 < hook 行索引）；模块级 `static ENV_MUTEX: Mutex<()>` 序列化 $PATH 改写（本模块唯一碰 `$PATH` 的测试）+ Drop 守卫恢复。

**验证**：按 cfg(unix) Windows 盲区规矩——临时把 `env/mod.rs` 的 `#[cfg(unix)] mod unix;` 放开 + 测试门控临时改 `#[cfg(test)]`，在 Windows 上强制编译并**实跑** unix 测试（5 passed：含既有 `bash_export_write_replace_read`/`bash_rebuild_line_add_path_entry`/`fish_per_dir_add_path_entry` + 两新测），验完两处门控改回。Windows 全量 lib 测试 23 passed 无回归。WSL 无 cargo 工具链，未端到端跑。

**改动文件**（1）：`crates/sdkcore/src/env/unix.rs`（`env/mod.rs` 临时改动已恢复，无净变化）。**未发版**。

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
- **WSL + Windows 双装 sdkm 的跨平台污染**（排查"项目级切换重启失效"教训）：WSL 默认 `appendWindowsPath=true` 继承 Windows PATH，但 **WSL bash 不补 `.exe` 后缀**（[WSL#2003](https://github.com/microsoft/WSL/issues/2003)）——bare `sdkm`/`node` 只命中文件名恰好叫 `sdkm`/`node`（无后缀）的条目，Windows 目录里的 `sdkm.exe`/`node.exe` **不匹配**。所以 Windows PATH 里的 `/mnt/d/.../sdkm` 是纯干扰，不能让 bare 命令生效。用户若在 WSL 敲 `sdkm.exe`（带后缀）则跑的是 **Windows sdkm**，操作 Windows home（`D:\...\sdkm`）、写 PowerShell profile/注册表，**完全不碰 Linux `.bashrc`**。排查要点：让用户 `command -v sdkm` + `file $(command -v sdkm)` 确认命中 Linux ELF 还是 Windows PE。`init` 的 PATH 持久化靠 `add_sdk_path`→`add_path_entry` 写 profile 行，但 `source_profile` 只把新 PATH 设进 **sdkm 子进程**（`env::set_var`），**不回传交互 shell**——所以 init/switch 后必须新开 shell 或 `source ~/.bashrc` 才生效。已修两 bug：dedup 不再回退进程 `$PATH`（Bug 1）、新建 PATH 行插到 hook 行之前（Bug 2，避免 source 时 hook 找不到 sdkm）。详见进度段本次改动。

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
