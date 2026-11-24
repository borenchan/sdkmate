# 命令详解

> 读者：普通用户查用法，贡献者查命令结构。

本文档详述 `sdkm` 的全部子命令：参数、别名、行为与示例。快速上手见 [usage.md](./usage.md)，配置项含义见 [configuration.md](./configuration.md)，自定义 SDK 见 [custom-sdk.md](./custom-sdk.md)，SDK 接入标准见 [sdk-onboarding.md](./sdk-onboarding.md)。

命令总览：

| 命令 | 别名 | 作用 |
|:---|:---|:---|
| [`sdkm init`](#sdkm-init) | — | 初始化 sdkm 运行环境 |
| [`sdkm install`](#sdkm-install) | `i` | 从远程安装 SDK 版本 |
| [`sdkm uninstall`](#sdkm-uninstall) | `rm`, `un` | 卸载本地已安装版本 |
| [`sdkm list`](#sdkm-list) | `ls`, `l` | 列出本地/远程版本（两层交互式 TUI） |
| [`sdkm switch`](#sdkm-switch) | `s` | 切换到本地已安装版本（全局） |
| [`sdkm use`](#sdkm-use) | — | 为当前项目 / 当前 shell 会话固定版本 |
| [`sdkm env`](#sdkm-env) | — | 输出当前目录应 eval 的环境脚本（hook 内部用） |
| [`sdkm hook`](#sdkm-hook) | — | 输出 shell hook 注册脚本（注入 profile 用） |
| [`sdkm current`](#sdkm-current) | `c` | 查看当前激活版本及其来源 |
| [`sdkm config`](#sdkm-config) | — | 配置管理（7 个子命令） |
| [`sdkm doctor`](#sdkm-doctor) | — | 打印诊断报告（提 issue 时粘贴） |
| [`sdkm clean`](#sdkm-clean) | — | 清理缓存与临时数据 |
| [`sdkm self`](#sdkm-self) | — | 管理 sdkm 自身（卸载 / 自更新） |

---

## sdkm init

首次使用前初始化 sdkm：创建 `store/`、符号链接目录、`config.toml`，把 sdkm 自身目录加入 PATH（让 `sdkm` 命令在任何终端可用），并注入 shell hook。

```bash
sdkm init           # 标准初始化（检测目录部署是否合理）
sdkm init --force   # 强制重新初始化：覆盖现有 config.toml，跳过目录检测
```

- `--force` / `-f`：强制重新初始化。会覆盖已有 `config.toml`，并跳过「目录部署检测」。
- 非 force 重跑**不覆盖**已有 `config.toml`，只补建缺失目录（改过 `symlink_dir` 后重跑可补建链接目录）。
- 首次生成配置时若检测到**中国大陆时区**，会自动启用 GitHub 下载镜像 `https://gh-proxy.com/`（随时 `sdkm config set network.github_download_mirror ""` 关闭）。
- 目标 shell 按登录 shell（`$SHELL`）检测；非登录 shell 场景（如 bash 登录后手动启动 fish）请在该 shell 的 profile 手动注入 hook，或把该 shell 设为登录 shell。
- 初始化流程会透明地逐步打印每一步操作及其用途，便于排查。
- Windows 下写入系统 PATH、环境变量与创建符号链接需**管理员权限**。

---

## sdkm install

从远程下载并安装指定 SDK 版本。安装后默认自动切换到该版本（除非加 `--no-switch`）。

```bash
sdkm install <SDK> <VERSION> [--no-switch]
```

- `<SDK>`：SDK 名称。内置 42 个（完整名单见 [custom-sdk.md · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)），也接受 [custom-sdk.md](./custom-sdk.md) 中注册的自定义 SDK。
- `<VERSION>`：目标版本，**支持模糊匹配**。例如 `21` 会解析为最新的 `21.x`，`20.11` 会匹配到 `20.11.x`。模糊命中时会先弹出确认（`Resolved '21' → '21.0.13'. Install this version?`），回车确认后才开始下载；精确版本不确认。TUI 里选的版本恒为精确版本，不触发确认。
- **Java 供应商**：默认装 Eclipse Temurin；换供应商把供应商写进版本号，两种写法等价——`21-zulu`（后缀式）与 `zulu@21`（前缀式）。别名：`tem`/`temurin`、`zulu`、`amzn`/`corretto`、`librca`/`liberica`。安装目录与 `current` 显示为规范形态 `21.0.13-zulu`（Temurin 无后缀，兼容旧数据）。`switch` / `uninstall` / `use` 同样接受这两种写法。
- `--no-switch`：仅安装，不自动切换到新版本。

示例：

```bash
sdkm install java 21            # 安装 Java 21 最新版（Temurin）并自动切换
sdkm install java zulu@17       # 安装 Zulu 的 17.x（等价于 17-zulu）
sdkm i node 20.11.0             # 安装指定 Node.js 版本并自动切换
sdkm install tomcat 11          # 安装 Tomcat 11.x（Apache 归档全历史）
sdkm install python 3.12 --no-switch   # 安装 Python 3.12 但不切换
```

> **Maven 说明**：Maven 没有远程版本发现接口，因此 `<VERSION>` 必须是精确版本号（如 `3.9.9`），不支持模糊匹配，也不支持 `list -r`。详见 [已知限制](./usage.md#known-limitations)。

安装流程内部拆分为 12 个阶段（解析 → 本地检查 → 构建 URL → 下载 → 解压 → 校验 → 标准化 → 安装验证 → 清理 → 自动切换…），各阶段带进度展示，失败时会自动回滚已完成的步骤。

---

## sdkm uninstall

从本地 store 删除已安装的版本（别名 `rm` / `un`）。

```bash
sdkm uninstall <SDK> <VERSION> [-y/--yes]
```

- `<SDK>`：SDK 名称（内置或自定义）。
- `<VERSION>`：要卸载的版本，**支持模糊匹配**（如 `21` → 本地已装的最新 `21.x`；TUI 里 `d`/`Delete` 也走这条路径）。
- `-y` / `--yes`：跳过交互确认（脚本/CI 用）。不加时先展示预检结果再确认。

**行为**（确认文案会说明将执行哪种分支）：

- 卸载**非激活**版本：仅删除该版本目录。
- 卸载**激活版本、且还有其他版本**：先把全局切换到一个其他已装版本，再删除目标目录。
- 卸载**唯一版本**：清理该 SDK 的符号链接、PATH 条目、环境变量与 `current_version`，再删除目录。
- 删除版本目录时若被其他进程/杀软占用，会部分完成并提示手动删除残留目录。

```bash
sdkm uninstall java 21          # 模糊命中 21.x 后确认卸载
sdkm rm node 18.20.4 -y         # 精确版本，跳过确认
```

---

## sdkm list

列出本地已安装版本或远程可用版本。**交互式 TUI 是默认形态**；非 TTY（管道、CI、agent）自动降级为文本输出。

```bash
sdkm list [SDK] [-r/--remote] [--limit N]
```

- 无 `<SDK>`：TTY 下进入**两层浏览 TUI**——第一层列出所有已注册 SDK（已安装在上、未安装在下，显示激活版本与占用空间），选中后进入第二层；退出第二层回到第一层。非 TTY 打印已安装 SDK 概览表（sdk / current / total）。
- `<SDK>`（本地）：直接进入该 SDK 的本地版本选择器。
- `<SDK> -r/--remote`：拉取远程版本列表后进入远程版本选择器。
- `--limit N`：**仅作用于非 TTY 的文本输出**（默认 20，必须 ≥ 1）。TUI 恒拉全量列表，靠 `f` 搜索筛选。
- `-r` 但未指定 `<SDK>` 会报错：`Please specify an SDK name`。
- `sdkm list maven -r` 会报错：Maven 不支持远程版本列表（自定义 SDK 未配 `version_url` 时同样）。
- `sdkm list java -r`：**四家供应商（Temurin/Zulu/Corretto/Liberica）的 LTS 系列 + 最新 feature 系列版本合并成一个列表**，非 Temurin 条目带后缀（如 `21.0.13-zulu`）；TUI 里按 `f` 搜 `zulu` / `amzn` / `librca` 即可筛。无 `--vendor` 参数——供应商就是版本号的一部分。
- `sdkm list tomcat -r`：解析 Apache 归档的 HTML 目录索引（root → `tomcat-9/10/11` 分支），含全部历史 patch，里程碑 `-M` 已过滤。

示例：

```bash
sdkm list                  # TTY：两层浏览 TUI；非 TTY：已安装概览
sdkm ls java               # 本地版本选择器
sdkm list node -r          # 远程版本选择器
sdkm list python -r | head # 非 TTY：远程文本列表（默认前 20 条）
sdkm list python -r --limit 50 > versions.txt   # 非 TTY 取 50 条
```

### 交互式 TUI 按键

**第一层：SDK 选择器**

| 按键 | 动作 |
|:---|:---|
| `↑` / `↓` 或 `k` / `j` | 上下导航 |
| `f` | 搜索 SDK 名（输入实时过滤，`Enter` 应用，`Esc` 清空） |
| `Enter` | 已安装 → 本地版本选择器；未安装 → 远程版本选择器（无版本发现源时提示并留在原地） |
| `r` | 直接进入远程版本选择器 |
| `Esc` | 有过滤时先清过滤，否则退出 |
| `q` / `Ctrl+C` | 退出 |

**第二层：版本选择器（本地 / 远程共用）**

| 按键 | 动作 |
|:---|:---|
| `↑` / `↓` 或 `k` / `j` | 上下导航 |
| `f` | 搜索版本号（输入实时过滤，`Enter` 应用，`Esc` 清空） |
| `Enter` 或 `s` | 切换到选中版本（需已安装） |
| `i` | 安装选中版本（仅远程选择器、仅未安装条目） |
| `d` 或 `Delete` | 卸载选中版本（需已安装；恒弹确认） |
| `Esc` | 有过滤时先清过滤，否则退出 |
| `q` / `Ctrl+C` | 退出（远程/本地选择器退出后回到第一层 SDK 选择器） |

列表项状态标记：`✅` 当前激活 / `📦` 已安装 / 空白 = 未安装。版本名后的 size 列在后台并行统计，冷启动时先显示 `…`，算完自动回填。

---

## sdkm switch

切换指定 SDK 的激活版本。目标版本必须已在本地安装。

```bash
sdkm switch <SDK> <VERSION>
```

- `<SDK>`：SDK 名称（内置或自定义）。
- `<VERSION>`：目标版本，必须是本地 `store/<sdk>/` 下已存在的版本；**支持模糊匹配**（如 `21` → 本地已装的最新 `21.x`），模糊命中时先确认再切换（`Resolved '21' → '21.0.13'. Switch to this version?`）。Java 供应商写法同样适用（`21-zulu` / `zulu@21`）。

示例：

```bash
sdkm switch java 21          # 切换到 Java 21
sdkm s node 20.11.0          # 切换到 Node.js 20.11.0
```

切换机制：创建/更新符号链接 `<symlink_dir>/<sdk_name>` → `store/<sdk>/<version>`，将符号链接的 bin 目录加入 PATH，并通过平台特定方式设置额外环境变量（如 `JAVA_HOME`），最后更新 `config.toml` 的 `current_version`。

**生效时机**：已注入 shell hook 时，切换完成**按一下回车**（提示符渲染触发 hook → `sdkm env` 重建 PATH）即生效，无需重启终端；未注入 hook 时需重启终端（或重新 source 托管环境文件 `<sdkm_home>/sdkm_env.sh`）。

**安全特性**：

- **PATH 冲突提示**：切换前检测 PATH 中是否已有同 SDK（按主命令名，如 `java`、`node`）的其他版本路径。命中时只发警告、不删任何条目——sdkm 注入的路径优先级最高，冲突路径会被自然覆盖。若发现"切了还是旧版本"，优先检查 PATH 里被警告的残留条目（`sdkm doctor` 可查）。
- **快照回滚**：切换过程中任一步骤失败（符号链接、环境变量、PATH、配置写入），都会自动回滚到切换前的状态——恢复旧符号链接目标、旧环境变量值、移除已添加的 PATH 条目、恢复旧配置。
- Windows 下创建符号链接与写入环境变量需**管理员权限**。

---

## sdkm use

为当前项目或当前 shell 会话固定一个 SDK 版本。与 `switch`（改全局符号链接）不同，`use` 走的是 shell hook 路径——**只影响当前目录及其子目录**，离开目录自动还原全局版本。

```bash
sdkm use <SDK> <VERSION> [--shell]
```

- `<SDK>`：SDK 名称（内置或自定义）。
- `<VERSION>`：目标版本，**支持模糊匹配**（如 `21` → 本地已装的最新 `21.x`）。
- `--shell`：仅对当前 shell 会话生效，不写文件，输出一段可 `eval`/`source` 的脚本。

### 项目级（默认）

```bash
sdkm use java 21          # 在当前目录写 .sdkm.toml：java = "21"
```

- 在当前目录生成（或合并进已有的）`.sdkm.toml`，把 SDK → **解析后的精确版本**作为 KV 摊平写入（如 `java = "21.0.2+9"`，模糊输入 `21` 自动解析，匹配逻辑与 switch 同源）。
- **要求本地已装且匹配成功**：版本本地未装或模糊匹配失败（如拼写错误）则拒绝写入并提示——坏 pin 会导致每次提示符渲染都解析失败降级全局。手写 `.sdkm.toml` 未装版本时 `sdkm env` 仍自动降级回全局并提示安装。
- 写入前若检测到上层目录已有 `.sdkm.toml`，会 warning 提示覆盖关系（只提示不阻断）。
- 写入后**按一下回车**（hook 触发 `sdkm env` 重读配置）即激活，无需重启终端或重新 cd。

### 会话级（`--shell`）

```bash
eval "$(sdkm use --shell java 21)"          # bash / zsh
sdkm use --shell java 21 | source            # fish
sdkm use --shell java 21 | iex             # PowerShell
```

- 不写文件，输出一行设置 `SDKM_ACTIVE_<SDK>` 环境变量的脚本（变量名由 SDK 名大写化得到，如 `SDKM_ACTIVE_JAVA`）。
- 会话级**优先级最高**，覆盖项目级与全局。
- 会话级版本**必须本地已安装**，否则报错退出（不像项目级那样宽容降级）。

### 四层优先级

> **临时环境变量**（`use --shell`）> **项目 `.sdkm.toml`**（`use`）> **全局符号链接**（`switch`，hook 每次回车从 config 动态推导）> **base 快照**（启动时的 PATH，兜底系统路径）——`sdkm env` 对每个 SDK 取第一个命中的层。
>
> 临时级 / 项目级依赖 shell hook 才会自动激活，**未注入 hook 时 `use` 不生效**，只能靠 `switch` 改全局。先跑 `sdkm init` 注入 hook。四层解析流程、hook 生效机制与流程图见 [工作原理](./usage.md#工作原理)。

---

## sdkm env

输出当前目录应 `eval`/`source` 的环境变量设置脚本。**由 shell hook 在每次提示符渲染时高频调用**，日常一般不手动跑。

```bash
sdkm env [--shell <shell>]
```

- `--shell <shell>`：指定目标 shell（`bash`/`zsh`/`fish`/`powershell`）；省略则自动检测当前 shell。
- 输出内容：PATH 外科手术重建行（读父 shell 实时 `$PATH` 与 `_SDKM_INJECTED_PATHS` 上次注入记录，剥旧注新：临时/项目 = store 真实版本目录，全局 = symlink 目录）+ 注入记录更新行 + 全局 known env vars 的幂等 unset + 本次选中 SDK 的 export（如 `JAVA_HOME`）。
- **外科手术重组**：只调整 sdkm 自己注入的条目，会话内其他 PATH 修改（venv/conda activate、手动 `export PATH`）原样保留。优先级按**分层**定位：会话/项目声明（`use --shell` / pin）恒顶最前，压过外部前插；全局层（switch 的 symlink）恒贴启动 PATH 边界，venv 等外部前插保持在它前面；删 pin / 离开项目后声明条目剥净，外部前插自然回到全局之前。重复执行不累积。
- 带双指纹缓存：`.sdkm.toml` 未改、会话变量与全局 active 状态未变时直接吐缓存脚本；任一变化（改配置 / `use --shell` / `switch`）即重算。未装降级等诊断走 stderr，不污染 stdout（stdout 必须纯净供 eval）。

手动验证当前目录会注入什么（不会改动环境）：

```bash
sdkm env --shell bash          # 看脚本内容
```

---

## sdkm hook

输出 shell hook 注册脚本。`sdkm init` 会自动把它追加进对应 shell 的 profile；本命令供手动注入或排查用。

```bash
sdkm hook [<shell>]
```

- `<shell>`：`bash`/`zsh`/`fish`/`powershell`；省略则自动检测。
- 输出的脚本注册一个「每次提示符渲染时执行 `sdkm env`」的钩子（bash 用 `PROMPT_COMMAND`、zsh 用 `precmd`、fish 用 `fish_prompt` 事件、PowerShell 包装 `prompt` 函数），并一次性保存启动 PATH 到 `_SDKM_BASE_PATH`。
- 消费方式按 shell 不同：bash/zsh `eval "$(sdkm hook bash)"`、fish `sdkm hook fish | source`、PowerShell `Invoke-Expression ((sdkm hook powershell) -join [Environment]::NewLine)`。

手动注入与重置方法见 [Shell 支持与 Hook 注册](./usage.md#shell-支持与-hook-注册)。

---

## sdkm current

查看当前生效的 SDK 版本及其**来源层**（会话 / 项目 / 全局）。

```bash
sdkm current [SDK]
```

- 无 `<SDK>`：打印 sdkm home，并以 `sdk / current / source` 三列表格展示所有 SDK 的生效版本。
- `<SDK>`：单行显示该 SDK 的版本与来源，如 `ℹ️  java 21.0.5 (global)`。

`source` 列取值（按 [四层优先级](./usage.md#四层优先级解析) 取第一个命中层）：

| 值 | 含义 |
|:---|:---|
| `session` | 来自 `sdkm use --shell` 的当前终端临时变量（优先级最高） |
| `project (路径)` | 来自向上找到的 `.sdkm.toml` 项目 pin |
| `global` | 来自 `sdkm switch` 写入的 `current_version` |

示例：

```bash
sdkm current         # 所有 SDK 生效版本 + 来源
sdkm c java          # 仅看 Java
```

> 某 SDK 在 store 里存在但未注册（config 中无条目）时，会在 stderr 提示 `skip unregistered SDK directory`，不参与展示。

---

## sdkm config

管理 `config.toml`。包含 7 个子命令。配置项的完整含义与类型校验规则见 [configuration.md](./configuration.md)。

```bash
sdkm config <subcommand> [args]
```

| 子命令 | 别名 | 作用 |
|:---|:---|:---|
| `set <KEY> <VALUE>` | — | 设置配置值（按类型校验后写入） |
| `get <KEY>` | — | 获取配置值（敏感值自动脱敏） |
| `list` | `ls`, `l` | 列出所有配置项 |
| `delete <KEY>` | `del` | 删除配置值（恢复默认；内置 SDK 不可删） |
| `edit` | `e` | 用系统编辑器打开配置文件，保存时校验 TOML |
| `add-sdk <NAME> ...` | — | 新增自定义 SDK 条目（详见 [custom-sdk.md](./custom-sdk.md)） |
| `remove-sdk <NAME>` | — | 移除自定义 SDK（内置 SDK 不可移除） |

`<KEY>` 使用点分隔格式，例如 `network.proxy`、`sdk.java.download_url`、`sdk.java.extra_vars.JAVA_HOME`。

示例：

```bash
sdkm config set network.proxy http://127.0.0.1:7890   # 设置 HTTP 代理
sdkm config get network.github_token                  # 读取 token（脱敏显示）
sdkm config list                                      # 列出全部配置
sdkm config delete network.proxy                      # 删除代理（恢复默认）
sdkm config edit                                      # 用编辑器打开 config.toml
```

**写入安全**：`set` / `delete` / `add-sdk` / `remove-sdk` 均采用**原子写入**（写入临时文件再重命名），操作失败时自动**快照回滚**到操作前的配置内容。

**内置 SDK 保护**：42 个内置 SDK（完整名单见 [custom-sdk.md · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)）的所有字段不可 `delete`，也不可 `remove-sdk`，只能通过 `set` 修改。

---

## sdkm doctor

打印只读诊断报告，专供提 issue 时粘贴。不改任何配置，每项独立探测、单项失败不中断。

```bash
sdkm doctor
```

报告分节：

| 分节 | 内容 |
|:---|:---|
| `version & platform` | sdkm 版本、平台三元组、系统版本 |
| `home & paths` | home / store / symlink / config 路径与存在性（`SDKM_HOME` 覆盖会高亮标注） |
| `shell & env persistence` | 检测到的 shell、各 profile 的 hook 注册状态、托管 env 文件（仅 Unix） |
| `network config` | 代理 / SSL 校验 / 超时 / 缓存 TTL / GitHub Token（敏感值只报 set/unset，不打印内容） |
| `installed SDKs` | 已安装 SDK 及其当前版本（标题带 `已安装 / 已注册` 总数） |
| `health checks` | 符号链接目标有效性、PATH 中是否有 sdkm links 条目、已装 SDK 的 `extra_vars` 实际值、`SDKM_HOME` 与 exe 目录是否一致 |

需要提 issue 时，把整份报告粘贴进 issue 正文即可。

---

## sdkm clean

清理缓存与临时数据，回收磁盘空间。**不碰** `store/`（已安装的 SDK）、`links/`、`config.toml`；被删的缓存按需自动重建。

```bash
sdkm clean            # 不带参数 = .cache 和 .tmp 都清
sdkm clean --cache    # 只清版本 API / 体积缓存
sdkm clean --tmp      # 只清下载、解压、自更新的临时文件
```

- `.cache`：版本列表接口缓存、目录体积缓存（`ls` 的 size）。
- `.tmp`：安装过程中间文件、自更新下载包与解压目录、替换失败留下的副本。
- 输出会分别报告 `.cache` / `.tmp` 释放的字节数与总计。

> 安装失败后想重来、或磁盘紧张时先跑 `sdkm clean`，比手动删目录安全。

---

## sdkm self

管理 sdkm 自身，含两个子命令。

### sdkm self uninstall

卸载 sdkm：清理所有被管理 SDK 的激活环境（符号链接 / PATH / 环境变量 / `current_version`），**移除当前 shell 的 profile 里的 sdkm hook 行**（固定注释 + hook 调用行，含 PowerShell legacy 形态）与**旧版直写进该 profile 的 sdkm PATH / 环境变量行**（只删路径在 sdkm home 或 `links/` 目录树内的行，精准匹配、宁可漏删），删除托管环境文件与 home 目录内容（store/links/.tmp/cache/config.toml）。config.toml 已不存在（此前卸载过/未 init）时会降级用默认配置继续清理残留。破坏性操作，强制交互确认、不可跳过。sdkm 二进制本身需手动删除（运行中的 exe 跨平台不可靠自删）。

### sdkm self update

从 GitHub Release 检查并替换 sdkm 二进制，CLI 与 GUI 始终一起更新、保持版本对齐。

```bash
sdkm self update              # 检查最新版，落后则下载替换（只升不降）
sdkm self update --check      # 只打印 current vs latest，不下载不替换
sdkm self update --rollback   # 回滚到"包含所需组合包的最新旧版本"
sdkm self u -c                # 别名 u + 短 flag -c / -r
```

- 别名 `u`；`--check`/`-c` 与 `--rollback`/`-r` 互斥，同时给会报错。
- **CLI + GUI 同步对齐**：同目录存在 `sdkm-gui` 时使用 `sdkm-desktop-*` 桌面组合包，一次替换两个二进制；纯 CLI 安装用 `sdkm-<platform>` 单包。目标版本缺少所需组合包时直接报错，绝不产生 CLI/GUI 版本错配。
- **只升不降**：升级检查时所有二进制都已是目标版本 → 提示已是最新、不下载。
- **权限预检 + 替换后验证 + 自动恢复**：碰网络前先在二进制目录试写（权限不足立即失败并提示提权）；替换后逐个 spawn `--version` 校验（旧版 GUI 不认识 `--version` 会打开窗口，用 3 秒超时兜底）；任一失败把原文件换回。
- **回滚不再依赖本地备份**：查询 release 列表，选「低于当前版本、且包含所需组合包的最新版本」，走与升级完全相同的下载/替换流程；没有可回退版本时报错说明原因。
- **产物命名**：CLI `sdkm-<platform>.<ext>`；桌面组合包 `sdkm-desktop-<platform>.<ext>`（gnu Linux / macOS / Windows 提供，musl 不提供桌面包）。
- **复用网络配置**：`network.proxy` / `ssl_verify` / `github_token` / `connect_timeout` / `github_download_mirror` 自动生效。
- **Windows 运行中替换**：全程 rename 不 delete（运行中的 exe 可 rename）；替换中断留下的 `.discard`/`.bad` 副本由下次 `sdkm clean` 或下次运行清理，不污染安装目录。
- **更新后 GUI 需重启**：有 GUI 时磁盘二进制已换新、但运行中的进程仍是旧版，GUI 诊断页进入「重启后生效」态（显示「立即重启」），期间抑制 CLI/GUI 版本不一致告警。
