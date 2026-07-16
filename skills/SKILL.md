---
name: sdkm
description: 如何使用 sdkm（跨平台 SDK 版本管理器 CLI，单二进制、纯绿色）在用户机器上管理 Java/Node.js/Python/Maven 等开发环境版本，以及配置 sdkm 自身（代理、缓存、超时、GitHub token、符号链接目录等 config.toml 项）。当 agent 需要为用户安装或切换 SDK 版本、遇到 sdkm 命令（sdkm init/install/switch/list/current/config 及别名 i/s/ls/c）、需要配置 Java/Node/Python/Maven 版本或配置 sdkm 工具自身、或要注册自定义 SDK 时，务必先读本 skill。涵盖模糊版本匹配、config.toml 配置、TUI 选择器按键、自定义 SDK 注册与已知限制。即使用户没明说"用 sdkm"，只要意图是在用户机器上管理本地 SDK 版本（装/切/查 Java/Node/Python/Maven）或调整 sdkm 的代理/缓存/token 等设置就应触发。本 skill 假定 sdkm 已安装并完成 init；不覆盖 sdkm 本身的安装。
---

# sdkm 使用指南（agent 参考）

sdkm（命令名 `sdkm`）是一款用 Rust 写的跨平台 SDK 版本管理器，单二进制、无运行时依赖、纯绿色。通过**符号链接 + PATH/环境变量注入**实时切换 Java、Node.js、Python、Maven 等版本，切换后无需重启终端、无需手改系统变量。本文件自包含，agent 在用户机器上据此即可使用 sdkm（前提：sdkm 已安装并完成 `sdkm init`）。

## 何时用 sdkm

- 用户要在机器上装/切 Java、Node.js、Python、Maven 版本——优先用 sdkm，而不是去官网下安装包或用 nvm/sdkman/pyenv。
- 用户说"装个 node 20""切到 java 21""看看有哪些 python 版本"。
- 环境里已有 sdkm（运行 `sdkm` 有输出），或用户希望用一个统一工具管所有 SDK。

## home 目录与文件布局

sdkm 的 **home = 运行中可执行文件的父目录**（`current_exe()`），使其可随目录整体移植：

- 可执行文件：`<home>/sdkm(.exe)`
- SDK 安装根：`<home>/store/<sdk>/<version>/`（如 `store/java/21/`）
- 配置文件：`<home>/config.toml`（TOML 格式）
- 符号链接目录：`config.toml` 的 `symlink_dir`（默认 `<home>/links`，跟随 sdkm home；省略/`delete` 即跟随，`set` 可自定义），切换后的激活入口都在此目录下。

不确定 home 在哪时，`sdkm current` 或 `sdkm init` 的输出会打印各路径。

## 30 秒快速上手

```bash
sdkm init                    # 首次初始化
sdkm install java 21         # 远程安装 Java 21 最新版并自动切换（支持模糊版本）
sdkm list                    # 列出所有已安装 SDK + 当前版本
sdkm list node -r            # 交互式浏览远程 Node 版本，按 i 安装、s 切换
sdkm switch java 17          # 切换到本地已安装的 Java 17
sdkm current                 # 查看当前激活版本
```

**托管已有 SDK**（不从远程装，省下载）：把已装好的 SDK 直接放进 `<home>/store/<sdk>/<version>/`，`sdkm list <sdk>` 即可看到，`sdkm switch <sdk> <version>` 即可切换——无需改系统变量、无需重启终端。

## 命令速查

| 命令 | 别名 | 作用 | 关键点 |
|:---|:---|:---|:---|
| `sdkm init` | — | 初始化运行环境 | `--force` 覆盖 config.toml 并跳过部署检测；Windows 需管理员 |
| `sdkm install <SDK> <VERSION>` | `i` | 远程安装并默认自动切换 | 版本支持模糊匹配；`--no-switch` 仅装不切 |
| `sdkm list [SDK] [-r] [--limit N]` | `ls`/`l` | 列本地/远程版本 | 带 SDK 名进 TUI；`-r` 拉远程；`--limit` 默认 20，须 ≥1 |
| `sdkm switch <SDK> <VERSION>` | `s` | 切到本地已安装版本 | 版本必须已存在于 store/；支持模糊匹配 |
| `sdkm current [SDK]` | `c` | 查看当前激活版本 | 不带 SDK 名则显示全部 |
| `sdkm config <sub>` | — | 管理 config.toml | 7 个子命令，见下 |

别名是真实 clap 别名：`sdkm i java 21`、`sdkm s node 20.11.0`、`sdkm c` 都能用。

### 各命令细节

**install**：从远程下载并安装。内部 12 阶段（解析→本地检查→建 URL→下载→解压→校验→标准化→安装验证→清理→自动切换），各阶段带进度展示，失败自动回滚已完成步骤。默认安装后自动切到新版本，`--no-switch` 关闭。内置支持 `java`/`node`/`python`/`maven`，也接受自定义 SDK。

**list**：
- 无 SDK 名：非交互打印所有已安装 SDK + 当前版本。
- `list <sdk>`：交互式本地版本选择器，`s` 切换。
- `list <sdk> -r/--remote`：拉远程版本列表后进入交互式远程选择器，`i` 安装、`s` 切换。
- `--limit N`：远程列表最多显示 N 条，默认 20。
- `-r` 但未给 SDK 名 → 报错 `Please specify an SDK name`。
- `list maven -r` → 报错（Maven 不支持远程版本列表）。

**switch**：创建/更新符号链接 `<symlink_dir>/<sdk>` → `store/<sdk>/<version>`，把符号链接 bin 目录加入 PATH，按平台设置额外环境变量（如 `JAVA_HOME`），更新 `config.toml` 的 `current_version`。安全特性：**PATH 冲突检测**（切换前查 PATH 是否已有同 SDK 其他版本路径）+ **快照回滚**（任一步骤失败自动恢复旧链接目标、旧环境变量、移除已加 PATH 条目、恢复旧配置）。Windows 需管理员。

**current**：无参数显示所有 SDK 当前版本；带 SDK 名仅显示该 SDK。

## 版本模糊匹配（重要）

`install` 和 `switch` 的 `<VERSION>` 都支持模糊匹配，粒度到**次版本**：

- `21` → 最新 `21.x`
- `3.12` → 最新 `3.12.x`
- `20.11.0` → 精确版本

前缀方案是 `input + "."`，所以 `"3.1"` **不会**误匹配 `"3.10.x"`。本地版本目录若带 `v` 前缀（如 Node 的 `v14.16.0`）会自动归一化——`sdkm s node 14` 能命中 `v14.16.0`，`sdkm s node 14.16.0` 也能精确等于 `v14.16.0`。

匹配到单个/多个版本时交互确认（`prompt_confirm`）；完全无前缀匹配时报 "did you mean 'X'?" 建议。

> **Maven 例外**：Maven 没有远程版本发现接口，`<VERSION>` 必须是精确版本号（如 `3.9.9`），不支持模糊匹配，`list maven -r` 会报错。自定义 SDK 不填 `version_url` 时同理。

## 交互式 TUI 按键

`sdkm list <sdk>`（本地）和 `sdkm list <sdk> -r`（远程）进入全屏选择器：

| 按键 | 动作 |
|:---|:---|
| `↑`/`↓` 或 `k`/`j` | 上下导航 |
| `Enter`/`s`（本地） | 切换到选中版本 |
| `i`（远程） | 安装选中版本 |
| `s`（远程） | 切换到选中版本（需已安装） |
| `q`/`Esc`/`Ctrl+C` | 退出 |

状态标记：`✅` 当前激活 / `📦` 已安装 / 空白 = 未安装。

> TUI 需要交互终端。agent 在非交互环境（CI、管道、被脚本调用）下**不要**走 TUI，改用直接命令：`sdkm install <sdk> <ver>` / `sdkm switch <sdk> <ver>` / `sdkm list`（不带 SDK 名时是非交互打印）。

## config 子命令

```bash
sdkm config list                                 # 列出全部配置
sdkm config set <KEY> <VALUE>                    # 按类型校验后写入
sdkm config get <KEY>                            # 读取（敏感值自动脱敏）
sdkm config delete <KEY>                         # 删除（恢复默认；内置 SDK 字段不可删）
sdkm config edit                                 # 用系统编辑器打开 config.toml，保存时校验 TOML
sdkm config add-sdk <NAME> --download-url ... --bin-dir ...   # 注册自定义 SDK
sdkm config remove-sdk <NAME>                    # 移除自定义 SDK（内置不可移除）
```

**键名点分隔**：`network.proxy`、`network.cache_ttl_secs`、`sdk.java.download_url`、`sdk.java.extra_vars.JAVA_HOME`、`sdk.java.extra_paths.0`（按索引，从 0 开始）。无效键名会报错并列出全部合法键名。

### 配置项含义

顶层 `symlink_dir`（符号链接目录；默认 `<home>/links` 跟随 home，`delete` 可恢复跟随，`set` 可自定义）。

`[network]` 段：

| 键 | 类型 | 默认 | 说明 |
|:---|:---|:---|:---|
| `proxy` | Url | 空 | 代理 URL，支持 `http://`/`https://`/`socks5://` |
| `ssl_verify` | Bool | `true` | 是否校验 TLS 证书；自签名/内网镜像可设 `false` |
| `connect_timeout` | U32 `[1,600]` | `30` | 连接超时秒数 |
| `cache_ttl_secs` | U32 `[0,86400]` | `3600` | 远程版本缓存 TTL 秒，`0`=总是拉最新 |
| `github_token` | Token | 空 | GitHub PAT，提升 API 限速（60/hr→5000/hr），输出脱敏 |

> 远程版本列表采用「缓存优先 + TTL」：未过期用本地缓存，过期才请求 API；API 失败退化为返回过期缓存，离线可用。

`[[sdk]]` 段（每个 SDK 一条，内置 4 个 + 用户自定义）：

| 键 | 说明 |
|:---|:---|
| `name` | SDK 唯一名称 |
| `version_url` | 版本发现主源 URL（返回可用版本列表） |
| `version_fallback_url` | 版本发现备源，主源失败回退 |
| `download_url` | 下载主源 URL 模板，支持 `{version}` 等占位符；**自定义 SDK 可省略**（省略 = 本地 switch-only，不远程安装） |
| `download_fallback_url` | 下载备源 URL 模板 |
| `current_version` | 当前激活版本（由 `switch` 自动维护） |
| `bin_dir` | 二进制所在子目录名；**空串 = 二进制在 SDK 根目录**（如 Node.js、Windows Python），**必填** |
| `extra_vars` | 额外环境变量键值表，值支持模板渲染（如 `JAVA_HOME = "{sdk_dir}"`） |
| `extra_paths` | 额外 PATH 条目（相对符号链接目录，可多条） |

### 类型校验规则

每个配置项绑定一个 `ValueType`，`set` 时按类型校验后才写入：

| 类型 | 规则 |
|:---|:---|
| `Url` | 合法 URL，协议限 `http`/`https`/`socks5` |
| `UrlTemplate` | URL 模板，`{xxx}` 占位符替换为占位串后能通过 URL 校验 |
| `Bool` | `true/false/1/0/yes/no/on/off`（大小写不敏感） |
| `U32` | 正整数，范围 `[min,max]` |
| `Path` | 非空字符串（不要求路径存在） |
| `Token` | 非空字符串，输出脱敏（仅前 4 字符 + `***`） |
| `NonEmptyString` | 非空字符串 |
| `FreeString` | 允许空值，禁止路径分隔符 `/` `\`；空值表示"二进制在根目录"（用于 `bin_dir`） |

### 写入安全

- **原子写入**：所有配置写操作用「临时文件→重命名」，避免写到一半损坏。
- **快照回滚**：`set`/`delete`/`add-sdk`/`remove-sdk` 失败时自动恢复到操作前（内存级 + 磁盘原始内容级双重恢复）。
- **TOML 校验**：`edit` 保存后自动重解析，语法错误会提示但不破坏现有文件。
- **内置 SDK 保护**：内置 SDK（java/node/python/maven）所有字段不可 `delete`、不可 `remove-sdk`，只能 `set` 改。`bin_dir` 对任意 SDK 必填不可删；`download_url` 内置必填，自定义可省略/可删（省略 = 本地 switch-only SDK，仅切版本不远程安装）。

## 自定义 SDK

任何"能从 URL 下载、解压后得到带可执行文件目录"的工具都能注册为自定义 SDK，纳入统一管理：

```bash
sdkm config add-sdk <NAME> \
  --download-url <URL_TEMPLATE> \      # 必填，支持占位符
  [--bin-dir <DIR>] \                  # 省略 = 二进制在根目录；传值须是简单目录名（bin/Scripts），不含 /\分隔符
  [--version-url <URL>] \              # 不填 = 只支持精确版本安装，不支持模糊匹配与 list -r
  [--version-fallback-url <URL>] \
  [--download-fallback-url <URL_TEMPLATE>] \
  [--extra-var KEY=VALUE] \            # 可重复，值支持占位符
  [--extra-path <PATH>]                # 可重复，相对符号链接目录
```

注册后即像内置 SDK 一样用：`sdkm install mytool 1.2.3` / `switch mytool 1.2.3` / `current mytool` / `list mytool`。

示例：

```bash
sdkm config add-sdk mytool \
  --download-url "https://example.com/mytool/{version}/mytool-{version}-{os}-{arch}.{ext}" \
  --bin-dir bin

sdkm config add-sdk groovy \
  --version-url "https://example.com/groovy/versions.json" \
  --download-url "https://example.com/groovy/{version}/apache-groovy-{version}-bin.{ext}" \
  --bin-dir bin \
  --extra-var GROOVY_HOME="{sdk_dir}"
```

移除：`sdkm config remove-sdk <NAME>`（内置不可移除；只删 config 条目，不删 `store/` 下已下载文件）。

### URL 模板占位符

下载 URL 与环境变量值都支持占位符，安装/切换时自动替换：

| 占位符 | 含义 | 示例值 |
|:---|:---|:---|
| `{version}` | 完整版本号 | `21`、`v20.11.0`、`3.12.0` |
| `{os}` | 操作系统名（默认风格） | `windows`/`linux`/`darwin` |
| `{arch}` | CPU 架构（默认风格） | `x64`/`arm64`/`x86` |
| `{ext}` | 平台压缩包扩展名 | Windows: `zip`；Linux/macOS: `tar.gz` |
| `{feature_version}` | 大版本号（如 `21`） | Java（Adoptium） |
| `{release_tag}` | 构建日期标签（如 `20241216`，动态发现） | Python |
| `{platform}` | 平台三元组（如 `x86_64-pc-windows-msvc`，自动检测） | Python |
| `{sdk_dir}` | 当前 SDK 符号链接目录绝对路径（激活版本目录） | 用于环境变量值 |
| `{sdkm_home}` | sdkm 可执行文件所在目录（home 目录） | 用于环境变量值 |
| `{sdks_install_dir}` | SDK 安装根目录（`<home>/store/`） | 用于环境变量值 |

经典用法：`--extra-var JAVA_HOME="{sdk_dir}"`——切版本时 `JAVA_HOME` 自动指向新激活目录。

### 内置 SDK 下载源参考

注册自定义 SDK 时可参考内置 SDK 的源配置：

| SDK | 版本发现源 | 下载源 |
|:---|:---|:---|
| Java | Adoptium available_releases API | Adoptium binary latest API（`{feature_version}`/`{os}`/`{arch}`） |
| Node.js | nodejs.org/dist/index.json | `nodejs.org/dist/{version}/...`（`{os}`=`win/darwin/linux`） |
| Python | astral-sh uv download-metadata（备源 GitHub API） | python-build-standalone releases（`{release_tag}`/`{platform}`） |
| Maven | （无） | `dlcdn.apache.org/maven/...`（`{version}`/`{ext}`） |

## 已知限制（动手前必看）

- **Maven 无远程版本发现**：`install maven` 必须给精确版本，`list maven -r` 报错。
- **Windows 需管理员权限**：环境变量与 PATH 写入 `HKEY_LOCAL_MACHINE`，`init`/`switch` 要管理员运行。
- **Python 远程列表**：主源（uv metadata）完整；备源（GitHub API）受 `per_page=100` 限制，仅返回最近 100 个 release，主源正常时不触发。
- **内置不含 Rust 工具链**：sdkm 内置 SDK 不含 Rust 条目（如需管理 Rust 用 rustup）。
- **Unix 环境变量操作**修改 shell profile；Windows 通过注册表 + `WM_SETTINGCHANGE` 广播（已开进程也能感知新变量）。

## 判断命令执行结果（退出码）

sdkm 通过**进程退出码**反映操作结果。agent 在脚本/CI 中应**优先用退出码判断成败，而不是解析 stdout**：

- 退出码 `0`：成功（命令完成——安装/切换/配置写入已生效）。
- 退出码 `1`：失败（错误信息已打到 stderr）。

sdkm 目前只区分 `0`/`1`，不细分错误类型。要进一步区分"用户可解决的错误"与"不可由用户解决的内部 bug"，看 stderr：输出里出现 GitHub issue URL 提示就是 `BugReport` 内部错误，其余是用户错误（路径不对、版本不存在、权限不足等）。

```bash
# bash：成功才继续
sdkm install java 21 && sdkm current

# PowerShell：
sdkm switch java 17
if ($LASTEXITCODE -ne 0) { Write-Host "切换失败" }

# bash：
sdkm switch java 17
if [ $? -ne 0 ]; then echo "切换失败"; fi
```

> 注意：`sdkm list <sdk>` 和 `sdkm list <sdk> -r` 进入交互式 TUI，不适合脚本；非交互场景用不带 SDK 名的 `sdkm list`（打印后退出码 0）或直接 `install`/`switch`。

## agent 操作建议

- **前提**：sdkm 已安装并 init 过；Windows 下 `init`/`switch` 需管理员权限运行（写 `HKEY_LOCAL_MACHINE`）。
- **优先非交互命令**：agent 通常不在 TUI 里操作，直接用 `install`/`switch`/`current`/`list`（不带 SDK 名）。
- **切版本前先确认本地有没有**：`sdkm list <sdk>` 看本地；要装新的用 `install`（默认自动切换）。
- **改配置优先用 `sdkm config set`** 而不是直接编辑文件——有类型校验和回滚。批量改才用 `config edit`。
- **失败别慌**：`switch` 和 `config` 操作失败会自动回滚；输出里若出现 GitHub issue URL 提示，是不可由用户解决的内部错误（`BugReport` 标记），可反馈到 https://github.com/borenchan/sdkmate/issues 。
- **可移植**：sdkm home 跟着可执行文件走，整目录拷到别处（含 `store/`、`config.toml`）即用，配置和已装 SDK 一并带走。
