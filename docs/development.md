# 开发者指南

> 读者：贡献者。

给准备改 sdkm 代码的人。协作流程、提交规范与 review 要求见 [CONTRIBUTING.md](../CONTRIBUTING.md)；本文只讲"怎么把项目跑起来、代码该放哪、怎么验证"。易变的历史坑、跨层契约与平台约束见 [development-notes.md](./development-notes.md)。

## 环境准备

- **Rust 1.98**（`rust-toolchain.toml` 固定版本，edition 2024）。GUI 依赖 gpui-kit 0.9，需要 stable 1.97+。
- **Linux** 需要 GUI 的原生依赖（与 CI 一致）：

  ```bash
  sudo apt-get install -y cmake pkg-config libx11-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libwayland-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libfontconfig1-dev libfreetype6-dev libglib2.0-dev libasound2-dev libvulkan-dev
  ```

  运行时还需要 `ca-certificates`（TLS 走 rustls + 系统 CA）。
- **macOS**：`brew install cmake`。
- **Windows**：MSVC 工具链即可。

## 构建与运行

工作区是四成员 virtual workspace，`default-members = ["crates/sdkm-cli"]`——**裸 `cargo build` 只编 CLI**，不连带 GUI。

```bash
cargo build                       # 调试构建 CLI（target/debug/sdkm）
cargo build --release             # 发布构建 CLI（target/release/sdkm）
cargo build -p sdkm-gui           # 单独构建 GUI（target/debug/sdkm-gui）
cargo build --release -p sdkm-gui # 发布构建 GUI
```

本地试跑（建议指向临时 home，别污染自己的环境）：

```bash
SDKM_HOME=/tmp/sdkm-dev ./target/debug/sdkm init
SDKM_HOME=/tmp/sdkm-dev ./target/debug/sdkm list
```

Windows PowerShell：`$env:SDKM_HOME="D:\tmp\sdkm-dev"; .\target\debug\sdkm.exe init`。

> ⚠️ `init` 会按真实行为向当前 shell profile 注入 hook、把 sdkm 目录写进 PATH（Windows 写注册表）。只是调试渲染/列表时，建议手工在临时 home 里放好 `config.toml` + `store/` + `links/`，只跑 `list` / `current` / `config` 等不碰环境的命令，避免污染自己的开发机。

## 质量门禁（与 CI 一致）

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace -j 2
```

- **fmt 阻塞**：CI 的 `Format check` 失败即红。
- **clippy 展示不阻塞**：仓库仍有预存警告（`collapsible_if` 等），CI 只打印、不 `-D warnings`。但**新增代码不应增加警告**。
- 单独跑某 crate：`cargo test -p sdkm-core`、`cargo test -p sdkm-util`、`cargo test -p sdkm-cli`。
- `clippy.toml` 开启 `absolute_paths`（`max-segments = 3`）：**使用处禁止跨模块全路径**，一律顶部 `use` 短名（`sdkm_util::builtin::find_seed(...)` 写体内 → `use sdkm_util::builtin::find_seed;`）；标准库同理，不写 `std::fs::` 全路径。

## 分层架构（动手前必读）

```
sdkm-util  ←  sdkm-core  ←  sdkm-cli
                        ←  sdkm-gui
```

- **sdkm-util**：无内部依赖的通用原语——SDK 类型与内置种子注册表、配置辅助、路径、shell backend、诊断数据。**不含终端渲染**。
- **sdkm-core**：全部业务。**零打印、零 stdin、零终端渲染依赖**。
  - 过程消息经 `sdkm_core::report`（`CoreEvent` / `EventSink` / `ProgressKind` / `NullSink`）通知前端；
  - 命令结果走结构化返回值，最终失败走 `Result::Err`（同一失败不再额外发 Error 事件）；
  - 确认交互归调用端：core 只产出决策数据（`resolve_install_version` / `resolve_installed_version` / `resolve_uninstall_target` / `check_init_env`），CLI 用 prompt、GUI 用弹窗确认后再调执行 API；
  - runtime 归前端持有：core 只出 `*_async` API；长阻塞段统一走 `sdkm_core::runtime::run_blocking`（内部 `spawn_blocking`）。**只有 `runtime.rs` 知道阻塞怎么跑**。
- **sdkm-cli**：clap 命令定义 + handler + 终端渲染（颜色/光标/prompt/表格/输出宏）。进程级单线程 runtime 在阶段边界 `block_on`。
- **sdkm-gui**：gpui-kit 桌面端。进程级 2-worker runtime 经 `data.rs` 的 `TaskId` 分发后台任务；页面在 `pages/`，文案在 `i18n/`；业务全部走 core。

**铁律检查**：新增功能按 core（数据）/ cli（展示）/ gui（展示）三层落位。`cargo tree -p sdkm-core -p sdkm-util` 不应出现 crossterm / unicode-width / indicatif。

## 常见改动放哪

### CLI 命令落位

用户文档（参数、示例、TUI 按键）见 [commands.md](./commands.md)。

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
| `sdkm doctor` | — | cli/DoctorHandler |
| `sdkm clean [--cache] [--tmp]` | — | cli/CleanHandler |
| `sdkm self uninstall` | — | cli/self_cmd::SelfUninstallHandler |
| `sdkm self update`（`--check`/`--rollback`） | `u` | cli/self_cmd::SelfUpdateHandler |

**新增 CLI 子命令**

1. `crates/sdkm-core/src/` 加业务方法与（如需要）结构化返回类型；
2. `crates/sdkm-cli/src/impls/<cmd>.rs` 实现 `CommandHandler`；
3. `crates/sdkm-cli/src/lib.rs` 的 `Commands` 枚举登记（命令名 / 别名 / about）；
4. `crates/sdkm-cli/tests/cli_golden.rs` 补集成断言。

**新增 GUI 页面/操作**

1. core 能力优先（缺 core API 先补 core，GUI 不自行拼业务）；
2. `crates/sdkm-gui/src/data.rs` 加 `TaskId` 分支；
3. `pages/` 渲染 + `i18n/` 中英文案（中文为默认语言，两套必须同步）；
4. 业务配置写 `config.toml`，界面偏好写 `gui.toml`。

**新增内置 SDK**：见 [sdk-onboarding.md · 贡献者新增内置种子](./sdk-onboarding.md#路径二贡献者新增内置种子)。种子按四类放在 `crates/sdkm-util/src/builtin/{lang,toolchain,cli,middleware}.rs`，追加到对应文件的 `SEEDS` 数组即可（`mod.rs` 的 `SDK_SEEDS` 自动汇总）；标准 B 用 `SdkSeed::gh()`、组合模式用 `SdkSeed::gh_cdn()`，不要手写结构体字面量。

## 测试约定

集成测试全部走**沙箱**，绝不触碰真实环境（注册表 / PATH / shell profile）：

- `SDKM_HOME` 指向临时目录（`SdkManager::new()` 等路径全经 `get_sdkm_home`，注入后全部重定向）；
- 环境操作注入 `MockEnv`（实现 `EnvOperation`，记录调用而不写真实环境）；
- `env::set_var` 是全局状态，**必须用模块级 `Mutex` 串行**；
- 测试结束在 `Drop` 里恢复环境变量并清理临时目录；
- 需要在某目录内跑 CLI 的测试，预写空 `.sdkm.toml` 截断向上查找，防止宿主机残留配置混入；
- config 里若是绝对路径的 `symlink_dir` 会写真实链接目录，测试必须同时隔离该配置。

现有测试：

| crate | 文件 |
|:---|:---|
| sdkm-core | `tests/{config,core_install_flow,current,env,install,link,self_uninstall,shell_integration,uninstall}.rs` |
| sdkm-cli | `tests/cli_golden.rs`（跑真实二进制 + SDKM_HOME 沙箱：exit code 精确、错误文案去 ANSI 全等、展示输出按关键 token 断言） |
| sdkm-util | `tests/{test_bug_report,test_util}.rs` |

- 新功能需提交测试锁定行为；**bug 回归的临时验证脚本跑通即删，不提交**。
- GUI 目前无单元测试，改 GUI 以手动启动验证为准。

## 平台相关代码

- `#[cfg(unix)]` / `#[cfg(windows)]` 分支在编译期裁剪——**改 unix 代码在 Windows 上不会编译**，类型错误会漏到 Linux 才暴露。
- 验证方法（Windows 本地）：临时把 `crates/sdkm-core/src/env/mod.rs` 的 `#[cfg(unix)] mod unix;` 改成 `mod unix;`（注释对应 `pub use` 避免冲突），`cargo check -p sdkm-core` 强制编译；验证完**必须恢复**并重新编译确认。或直接在 WSL 里 `cargo build`。
- `cargo check --target x86_64-unknown-linux-gnu` 在 Windows 上通常不可行（aws-lc-rs 找不到交叉编译器）。
- 涉及跨平台代码提交前，保证 Windows + Linux 双端编译通过。

## 错误与诊断约定

- 全项目统一 `anyhow`（`bail!` / `.context()`），不用 thiserror。
- `BugReportError` 标记只用于**用户无法自行解决**的错误（解压/校验失败、内置配置缺失），CLI 检测到会提示提 issue；权限、环境问题用普通错误。
- debug 构建的 `error!` 会额外打印 backtrace。
- core 内的 `sink.warning/info/...` 文案属于跨层契约：例如 `bin dir mismatch for \`<sdk>\`` 是 GUI 翻译层解析 SDK 名的锚点，改前缀必须同步改 `crates/sdkm-gui/src/i18n/errors.rs`。

## 发版相关（维护者）

- **不要改根 `Cargo.toml` 的版本号**、**不要改 `.github/workflows/`**——发版由维护者控制，CI 与七产物发布链同样是维护者维护。
- 提交后推送分支开 PR 即可；合并后发版时 commit 自动进入 Release 正文的 changelog（按 conventional commits 分组，不再回写根 `CHANGELOG.md`）。

## 排查常用

```bash
cargo build -p sdkm-cli && ./target/debug/sdkm doctor   # 诊断自己沙箱的 sdkm
SDKM_HOME=/tmp/x ./target/debug/sdkm config list        # 看沙箱配置
cargo clippy -p sdkm-core --message-format=short        # 只看当前 crate 的警告
RUST_BACKTRACE=1 ./target/debug/sdkm list               # 强制 backtrace
```

遇到行为不符合预期，先看对应 core 模块的注释（`switch.rs` / `install/mod.rs` / `env/` 等文件头部有设计说明）。