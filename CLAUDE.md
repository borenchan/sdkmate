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

## 当前开发进度（2026-09-04）

### 2026-09-04 —— PS unset 行 Remove-Item → $null 赋值（v0.4.5）

用户反馈 Windows PowerShell profile 加载 803ms 疑似 hook 耗时。实测定位：803ms = PS 5.1 基线 ~310ms + spawn sdkm hook ~150ms + IEX hook ~140ms + hook 内 spawn sdkm env ~150ms + IEX env 输出 ~140ms；spawn 慢的主因是机器装有奇安信天擎实时防护（裸 cmd spawn 都要 130-200ms）。代码侧可做的优化点：`Remove-Item Env:X` 每行 37-150ms（走 PSDrive provider）→ `$env:X = $null` 0.3ms（PS 5.1/7 语义一致：赋 null 即删除），extra_vars 多时 unset 行数线性放大收益。

**改动文件**（3）：`crates/util/src/shell_backend/pwsh.rs`（unset_line）、`crates/util/src/shell_backend/mod.rs`（注释）、`crates/sdkcore/src/shell/env.rs`（PS golden 测试）。**发版 v0.4.5**。

**性能分析结论备忘**（本机实测）：IEX 单次固定开销 35-150ms（PS 5.1 逐次编译无缓存）；`$env:X = $null` 与 `Remove-Item Env:X` 语义等价已端到端验证。剩余大头在环境侧：天擎白名单（收益最大）、装 PS 7。profile 内联 hook 函数可再省 ~200-350ms 但放弃模板热更新，未采纳。

### 本次改动 —— hook 全局层动态化：switch 后按回车即生效，无需重启终端（未发版未 bump）

用户指出：既然已有 hook 机制（每次 prompt 触发 `sdkm env` 重建 PATH），全局层理应动态反映当前 active 状态，而不是每次都要求重启终端。定位：三层解析里会话层/项目层都是动态的，唯独全局层是静态残留——输出「还原启动 base 快照」（`set -gx PATH $_SDKM_BASE_PATH`），switch 中途新写入的 symlink bin 不在快照里，每次回车反而被冲掉（WSL fish「switch 后 node not found」的根因之一）。机理背景：任何外部 CLI 子进程都无法修改父 shell 环境（`source_profile` 的 `env::set_var` 只改 sdkm 自己进程），hook 的 source 是用户 shell 执行的、能改 PATH——所以「自动生效」的正道是让 env 输出正确的 PATH，而不是让程序去 source。

**修法（三件套）**：
1. **全局层动态化**（`shell/env.rs`）：新增 `global_active_sdks(covered)`——config 里 `current_version` 已设且未被会话/项目层覆盖的 SDK → `SelectedSdk{bins: <symlink_dir>/<sdk>[/<bin_dir>] + extra_paths, env_vars: extra_vars 渲染（{sdk_dir}=symlink 目录，与 switch 持久化语义一致）}`，`selected.extend` 追加在会话/项目层之后（同 SDK 多层被 covered 去重，各 SDK 只出现一次，PATH 顺序无功能冲突）。效果链：switch 改 config + symlink → 按回车 → hook 触发 env → 全局层输出新 bin → **立即生效**。java 全局 active 时 JAVA_HOME 也不再被 known-unset 冲掉（顺带修复：旧实现全局层不出 extra_vars，known 集合每次回车 unset JAVA_HOME）。项目层同 SDK 重复 pin 加 `pinned_names.insert` 去重（取第一个）。
2. **缓存全局指纹**（`hook_cache.rs`）：`global_fingerprint()` = `symlink_dir|name=version;...`（active SDK 按名排序；config 读失败返空串静默降级）。`resolve` 增判据（指纹由调用方传入，本进程只算一次，判据+put 采样复用）+ `HookEntry.global_fingerprint` 字段（serde default）+ **schema 4→5 bump**（旧条目指纹空串会与「无 active」状态误命中）。与上轮会话指纹同款模式。
3. **提示语**（`switch.rs`）：`Please restart your terminal` → `Press Enter to apply, or restart your terminal if hooks are not installed.`。init 提示保持不变（首次 init 后 hook 尚未在本会话生效，重启仍必要）。

**IO 性能核查**（用户要求确认）：hook 命中热路径新增 IO 只有 `global_fingerprint()` 读 config.toml + TOML 解析（<1ms，典型 config 2~5KB；resolve 短路顺序 shell/schema 在前，前三项不匹配不付此成本）；对比 sdkm 进程 spawn 本身 5~20ms、调用频率 ≤1 次/秒，无感知。缓存 miss 路径原实现指纹算两次（resolve+put），已改为调用方算一次传入 `resolve(pwd, shell, &gf)` 复用。hook_cache.json 仅多存一个几十字节字段，prune 机制不变。

**验证**：临时集成测试 2 个（全局层 bins+JAVA_HOME 出现在脚本 / switch 改 config 写盘后同 PWD 缓存重算出新 bin——测试里重建 manager 忠实模拟 hook 新进程重读 config），按规矩验证后已删除。Windows 真机端到端（用户验证）：`sdkm env --shell powershell` 输出优先级正确（项目层 node=store 真实目录 + 全局层 java/go=links bins + JAVA_HOME=links\java），终端输出正确。workspace 16 测试目标 ok；clippy 改动文件零警告。

**自审修正的两处**（初版方案的错误，review 发现）：① 初版把全局 bins 放 PATH **最前**——违反「项目 > 全局」优先级语义，改为全局层 extend 到 selected 尾部；② 初版全局层只出 bins 不出 extra_vars——java 全局 active 时 JAVA_HOME 每次回车被 unset，补渲染。

**改动文件**（4）：`crates/sdkcore/src/shell/env.rs`（global_active_sdks + 优先级拼接 + 指纹复用）、`crates/sdkcore/src/hook_cache.rs`（全局指纹 + schema 5）、`crates/sdkcore/src/switch.rs`（提示语）、`crates/sdkcore/tests/shell_integration.rs`（字面量补字段）。release 已覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（.bak 备份）。**未发版未 bump**（等 WSL fish 实测后再 bump）。

### 2026-08-31 —— fish PATH 行幂等化 + 插到 hook 注释之上（v0.4.4）

### 2026-08-31 —— fish PATH 行幂等化 + 插到 hook 注释之上（v0.4.4）

用户复测 v0.4.3 relocate 修复暴露两个体验问题：(1) PATH 行插在 `# sdkm project-level version hook` 注释与 hook 调用行之间，应落在注释上面更清晰；(2) 重复 switch 时 node 行与 root 行相对顺序来回翻转——上轮 relocate 语义无条件删行重插的副作用，用户指出应「已存在就不写入」。

**修法**：① 新增 `find_hook_block_start`（marker 行向上回溯过固定注释；注释字面量抽成 `util::consts::HOOK_COMMENT_LINE`，inject.rs 写入与 unix.rs 回溯共用同一常量防漂移），RebuildLine 与 PerDirCommand 两分支统一改用它插行。② PerDirCommand 增幂等判据：同串行已存在且在 hook 块之前 → no-op + warning（与 bash `already_in_profile` 行为对齐）；仅错位在 hook 之后的旧污染行仍 relocate 自愈；行存在但无 hook 块 → 视为已就位。

**验证**：临时放开门控（RUSTFLAGS `--cfg temp_verify`）Windows 实跑 unix 测试 9 passed（4 个临时测试：插到注释之上/fish 幂等 no-op/错位行仍自愈/bash 同步受益），按规矩验完删除、diff 只留修复。workspace 16 个测试目标 ok；clippy 本次改动文件零警告（修掉本次引入的 `single_char_add_str`）；fmt 干净；release 已覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（.bak 备份）。

**改动文件**（3）：`crates/sdkcore/src/env/unix.rs`（find_hook_block_start + PerDirCommand 幂等判据）、`crates/sdkcore/src/shell/inject.rs`（注释行走共享常量）、`crates/util/src/consts.rs`（HOOK_COMMENT_LINE）。**发版 v0.4.4**。

### 2026-08-31 —— 修 fish PerDirCommand PATH 行错位到 hook 行之后（Bug 2 同型，v0.4.3）

用户 WSL fish 实测：`sdkm s node 16` 后 config.fish 里 `fish_add_path` 行落在 `sdkm hook fish | source` 之后，重启终端 `node -v` 仍 not found。机理与 bash Bug 2 同型：hook 行先跑 → `_SDKM_BASE_PATH` 快照固化时不含 node bin → 每次 prompt 触发 `sdkm env` 重建 PATH（`set -gx PATH $_SDKM_BASE_PATH`）把 bin 冲掉。上次修复只改了 RebuildLine 分支，PerDirCommand（fish）分支漏掉——两分支代码路径独立，同类改动必须两边都看。

**修法（relocate 语义）**：PerDirCommand 分支由「追加末尾」改为「删旧位置同串行 + 插到 hook marker 之前（`find_hook_marker_line` 复用，无 marker 则追加）」。一步解决三件事：新插入位置正确、重复 init 不再累积重复行、已污染 profile 重跑 switch 即自愈（错位行被删重插）。

**验证**：临时放开门控（env/mod.rs + unix.rs 测试门控改 `any(unix, temp_verify)`，RUSTFLAGS `--cfg temp_verify`）在 Windows 实跑 unix 测试 7 passed（含 2 个新 fish 回归测试：插到 hook 前 / relocate 自愈）。按用户规矩 bug 回归测试用临时文件验证后删除、不提交，本次 diff 只剩修复本身（15 行）。workspace 全量 16 个测试目标 ok 无回归；clippy 本次改动文件零警告（既有警告在 config_helper.rs/discovery.rs 非 本次引入）；fmt 干净。已 build release 并覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（先备份 .bak）。

**改动文件**（1）：`crates/sdkcore/src/env/unix.rs`（PerDirCommand relocate；`env/mod.rs` 临时改动已恢复无净变化）。**发版 v0.4.3**（bump 根 Cargo.toml + Cargo.lock，patch：纯代码修复）。

### 2026-08-28 —— 修 hook 缓存打破「会话 > 项目」优先级（会话指纹判据）

用户实测暴露：项目 pin（`sdkm use node 16` 写 `.sdkm.toml`）后跑 `sdkm use --shell node 25`（父 shell 设 `SDKM_ACTIVE_NODE`），同目录下 `node -v` 仍是项目版本，`cd` 出去才变 v25——与文档「临时 > 项目 > 全局」矛盾。定位：**不是解析逻辑错**（`shell/env.rs` 三层解析本就 session 先查、project 遇 session 已覆盖直接跳过），而是 **hook 缓存 key 漏了会话状态**：`hook_cache.rs` 命中判据只有 PWD + `.sdkm.toml` mtime + shell + schema，`use --shell` 只在父 shell 设环境变量、不碰缓存 → 下一条 prompt hook 命中旧缓存吐项目版本脚本。同 PWD 缓存 miss 的时机（首次 cd 进 / mtime 变）反而"碰巧"正确，故用户看到"项目目录里 shell 覆盖不生效、离开目录才生效"。

**修法（指纹比对）**：`HookEntry` 增 `session_fingerprint` 字段（当前进程所有 `SDKM_ACTIVE_*` 变量按名排序拼 `name=value;` 连接，空集=空串；`current_session_fingerprint()` 公开函数，env.rs put 时采样、测试直接引用）。`resolve` 增判据：条目指纹 ≠ 当前指纹 → miss 重算。覆盖 set/改/unset 全路径，不依赖各 setter 记得失效缓存。**schema 3→4 bump 必要**：旧条目 `serde(default)` 指纹为空串，与"当前无会话变量"指纹相同，不 bump 会误命中 bug 时期旧条目。

**回归测试**：`env_script_recomputes_when_session_var_set`（shell_integration.rs）——先缓存项目 pin(21) 脚本 → 设 `SDKM_ACTIVE_JAVA=25` → 断言重算出 25 bins 且无 21（复现用户 bug 场景）；Drop 守卫 + 前置 remove_var 防真实会话变量污染测试。既有 `hook_cache_cross_shell_miss`/`hook_cache_schema_mismatch_miss` 的 HookEntry 字面量补指纹字段（跟随当前指纹 = 模拟同状态命中）。

**验证**：workspace 全量 59 passed 无回归（sdkcore lib 23 + shell_integration 8 + 其余）；clippy 本次改动文件零警告（既有警告非本次引入）。已 build release 并覆盖 `D:\develop\sdk\.sdkm\sdkm.exe`（备份 .bak），用户可直接复测原日志场景。**未发版**。

**改动文件**（3）：`crates/sdkcore/src/hook_cache.rs`（指纹函数+判据+schema bump）、`crates/sdkcore/src/shell/env.rs`（put 采样指纹）、`crates/sdkcore/tests/shell_integration.rs`（回归测试+字面量补字段）。

### 2026-08-27 —— 修 `env/unix.rs` 两个 PATH 持久化 bug

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
- **env/unix.rs 的 RebuildLine 与 PerDirCommand 是两条独立代码路径**：改 PATH 持久化行为时（插入位置、去重、删除），bash/zsh 走 RebuildLine 分支、fish 走 PerDirCommand 分支，**修一处必须检查另一处是否同型遗漏**——bash Bug 2（PATH 行错位到 hook 后）修了 RebuildLine、漏了 PerDirCommand，fish 用户 WSL 实测才暴露（v0.4.3 补修）
- **bug 回归测试用临时文件验证后删除，不提交**：用户规矩——测试代码跑通证明修复生效即可，diff 里只留修复本身
- **WSL + Windows 双装 sdkm 的跨平台污染**（排查"项目级切换重启失效"教训）：WSL 默认 `appendWindowsPath=true` 继承 Windows PATH，但 **WSL bash 不补 `.exe` 后缀**（[WSL#2003](https://github.com/microsoft/WSL/issues/2003)）——bare `sdkm`/`node` 只命中文件名恰好叫 `sdkm`/`node`（无后缀）的条目，Windows 目录里的 `sdkm.exe`/`node.exe` **不匹配**。所以 Windows PATH 里的 `/mnt/d/.../sdkm` 是纯干扰，不能让 bare 命令生效。用户若在 WSL 敲 `sdkm.exe`（带后缀）则跑的是 **Windows sdkm**，操作 Windows home（`D:\...\sdkm`）、写 PowerShell profile/注册表，**完全不碰 Linux `.bashrc`**。排查要点：让用户 `command -v sdkm` + `file $(command -v sdkm)` 确认命中 Linux ELF 还是 Windows PE。`init` 的 PATH 持久化靠 `add_sdk_path`→`add_path_entry` 写 profile 行，但 `source_profile` 只把新 PATH 设进 **sdkm 子进程**（`env::set_var`），**不回传交互 shell**——所以 init/switch 后必须新开 shell 或 `source ~/.bashrc` 才生效。已修三 bug：dedup 不再回退进程 `$PATH`（Bug 1）、新建 PATH 行插到 hook 行之前（Bug 2，避免 source 时 hook 找不到 sdkm）、fish PerDirCommand 分支同型错位 relocate 自愈（v0.4.3）。详见进度段本次改动。

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
