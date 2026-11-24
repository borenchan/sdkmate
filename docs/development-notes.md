# 开发注意事项（易变）

> 读者：贡献者。

本文收录 sdkm 开发中的历史坑、跨层契约与平台约束。内容会随排查持续补充——**新条目直接追加到本文件，不要改 `CLAUDE.md`**。

改以下模块前，先按关键词检索本文：

- 安装 / 解压 / 目录布局 → `install`、`normalize_extracted_dir`、`bin_dir`、`zip`
- 环境变量 / Shell → `EnvOperation`、`env/unix.rs`、`$SHELL`、WSL
- 版本发现 / 下载源 → `version/`、`discovery`、Tomcat、Java 供应商
- GUI → `gpui-kit`、`on_ok`、`i18n/errors`
- 测试 → `SDKM_HOME`、`MockEnv`、沙箱
- 构建 / 依赖 → `rust-toolchain.toml`、`reqwest`、`zip`


- **集成测试禁止触碰真实 shell profile（2026-11-20 事故）**：`uninstall_self` 的 hook 行移除必须走 `EnvOperation::remove_shell_hook`（trait 默认空操作，测试 MockEnv 天然隔离）；历史上它直接调真实 `shell::inject::remove_shell_hook`，`tests/self_uninstall.rs` 每跑一次 `cargo test` 都会删掉用户真实 PowerShell profile 的 hook 行（已修复）。**新增任何操作 profile/注册表/PATH 的路径都必须经过 EnvOperation**，否则测试会污染真实环境
- **gpui-kit 确认框 `on_ok` 里不能直接开新弹窗**：确认框的延迟关闭（`defer_close_dialog`）关的是"最顶层非关闭弹窗"——`on_ok` 里新开的进度窗会被它连带关掉（表现为"点确认后没有进度反馈"）。必须在 `on_ok` 里 `window.defer` 延后开，并检查忙碌标志（瞬时操作的 Done 可能先到，防残留弹窗）
- **bin_dir 布局警告文案是跨层契约**：core 侧前缀 `bin dir mismatch for \`<sdk>\`` 是 GUI 翻译层（`errors::translate`）解析 SDK 名的锚点，改动前缀必须同步改 `errors.rs`；install 校验与 `import_local_version` 两处同口径
- **SDK 能否远程安装统一口径（2026-11-20 决策）**：一律走 `SdkConfig::can_install_remotely()`（download_url 或 version_url 至少一个非空；GitHub Releases 形态仅填 version_url 也能装，版本发现会产出资产直链），`RegisteredSdkItem.can_install` 透传 GUI。**CLI/GUI 禁止自行按单字段判断**——历史错误：CLI 按 `download_url.is_some()` 把 GitHub 引擎型 SDK 误报成 switch-only，GUI 按 `has_version_url` 会误隐藏 Maven 的精确版本安装入口。两者皆无 = 本地 switch-only，GUI 隐藏"安装新版本"、改用"导入本地版本"
- **导入本地版本 = 移动语义（2026-11-20 决策）**：`import_local_version` 把用户既有版本目录整体移入 `store/<sdk>/<version>`（跨盘自动 copy+delete），**不做 store 内软链**——uninstall 用 `remove_dir_all`、size 统计/空目录清理全按"目录在 store 内"假设，软链会误删用户源目录；且 `symlink_dir/<sdk>` 本身已是指向 store 的软链，再套一层无收益。导入不改目录内部布局（用户选的目录即版本根），bin_dir 不可见只发软警告（同 install Phase 9b）；版本名即目录名，须唯一且无路径分隔符（防路径穿越），目标已存在直接拒绝
- **自更新临时文件兜底**：`self_update.rs` 用 `TempGuard`（Drop 清理）保证下载/解压中途失败（`?` 提前返回）不留半截包与解压目录；成功路径 disarm 后走 `run_blocking` 异步清理。运行中 GUI 的旧副本（`.discard`）Windows 删不掉，留给下次 `clean_leftovers`
- **自更新完成后的 GUI 状态**：磁盘二进制已替换但进程仍是旧版，`GuiApp.restart_required` 进入"待重启"态——诊断页显示"立即重启"按钮并抑制 CLI/GUI 版本不一致报警（两者差异属预期），重启成功后自然消失。回退（Rollback）与"磁盘已最新但进程落后（UpToDate）"同样进入该状态
- **SDK 包布局三形态归一化（2026-11-19 决策）**：`normalize_extracted_dir` 以「bin_dir 在版本目录可见」为锚点依次处理三种已知包形态——单层包装（node/go/maven）、双层包装（python-build-standalone）、macOS JDK bundle（`jdk-x/Contents/Home`，摊平 Home 内容并清掉 MacOS/_CodeSignature/Info.plist 残壳；bin_dir 锚点检测驱动、无平台 gate）。**不做通用递归搜索摊平**：第四种形态出现时由 install Phase 9b 的软警告（`locate_bin_dir` 限深搜出 bin 实际位置 + `sdkm config set sdk.<name>.bin_dir` 修正提示）提前暴露。摊平后 sdkm 管的 JDK 不再被 `/usr/libexec/java_home` 识别（sdkm 用户走 PATH+JAVA_HOME，文档已注明）
- **zip 解压必须恢复 unix_mode 权限位**：bun/deno/pnpm/uv 等 GitHub 引擎 SDK 在 mac/Linux 全是 zip 分发，可执行文件打包即 755；`fs::File::create` 默认 644 → permission denied。`extract_zip` 已按 `entry.unix_mode()` 还原（cfg(unix)；tar.gz 走 `archive.unpack()` 自带恢复）。WSL `/mnt/*`（drvfs）权限位被挂载层抹平全 0777，掩盖此 bug——WSL 测不出，须 mac/原生 ext4 验证
- **本地版本枚举过滤隐藏条目与文件**：`list_local_sdk_versions`/`effective_versions_all`/`cleanup_empty_sdk_store_dir` 三处均已过滤 `.` 开头条目与非目录（macOS Finder 的 `.DS_Store` 曾被当成版本号）；判空清理用 `remove_dir_all`（忽略隐藏条目后目录可能非空）
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
- **env/unix.rs 的 RebuildLine 与 PerDirCommand 是两条独立代码路径**：改托管 env 文件的 PATH 行行为时（插入位置、去重、删除），bash/zsh 走 RebuildLine 分支、fish 走 PerDirCommand 分支，**修一处必须检查另一处是否同型遗漏**——bash Bug 2（PATH 行错位到 hook 后）修了 RebuildLine、漏了 PerDirCommand，fish 用户 WSL 实测才暴露（v0.4.3 补修）
- **Unix 环境持久化只走托管 env 文件**：不要在 `switch`/`uninstall` 等命令里直接编辑用户 profile；profile 只由 `init` 注入 source/hook 行、由 `self uninstall` 精准移除。自卸载的清理**只作用于当前 shell（`$SHELL`）**，且整行精确匹配（注释行/hook 调用行/legacy 形态；PATH 条目必须等于 `<home>` 或在 `links/` 树内才删），宁可漏删不误删
- **shell 检测只看 `$SHELL`（2026-11-18 决策）**：不引入父进程检测——`/proc` 仅 Linux、会覆盖正确的 `$SHELL`、为"非登录 shell 里手动启动 fish"这种边缘场景伤正常场景；该场景靠手动注入 hook 或把目标 shell 设为登录 shell（文档已注明）
- **bug 回归测试用临时文件验证后删除，不提交**：用户规矩——测试代码跑通证明修复生效即可，diff 里只留修复本身
- **WSL + Windows 双装 sdkm 的跨平台污染**（排查"项目级切换重启失效"教训）：WSL 默认 `appendWindowsPath=true` 继承 Windows PATH，但 **WSL bash 不补 `.exe` 后缀**（[WSL#2003](https://github.com/microsoft/WSL/issues/2003)）——bare `sdkm`/`node` 只命中文件名恰好叫 `sdkm`/`node`（无后缀）的条目，Windows 目录里的 `sdkm.exe`/`node.exe` **不匹配**。所以 Windows PATH 里的 `/mnt/d/.../sdkm` 是纯干扰，不能让 bare 命令生效。用户若在 WSL 敲 `sdkm.exe`（带后缀）则跑的是 **Windows sdkm**，操作 Windows home（`D:\...\sdkm`）、写 PowerShell profile/注册表，**完全不碰 Linux `.bashrc`**。排查要点：让用户 `command -v sdkm` + `file $(command -v sdkm)` 确认命中 Linux ELF 还是 Windows PE。`init`/`switch` 的 PATH 持久化写托管 env 文件，`source_env_file` 只把新 PATH 设进 **sdkm 子进程**（`env::set_var`），**不回传交互 shell**——所以 init/switch 后必须新开 shell 或 `source` 托管 env 文件才生效。已修三 bug：dedup 不再回退进程 `$PATH`（Bug 1）、新建 PATH 行插到 hook 行之前（Bug 2，避免 source 时 hook 找不到 sdkm）、fish PerDirCommand 分支同型错位 relocate 自愈（v0.4.3）。

