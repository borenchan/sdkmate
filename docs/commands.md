# 命令详解

本文档详述 `sdkm` 的全部子命令：参数、别名、行为与示例。快速上手见 [usage.md](./usage.md)，配置项含义见 [configuration.md](./configuration.md)，自定义 SDK 见 [custom-sdk.md](./custom-sdk.md)。

命令总览：

| 命令 | 别名 | 作用 |
|:---|:---|:---|
| [`sdkm init`](#sdkm-init) | — | 初始化 sdkm 运行环境 |
| [`sdkm install`](#sdkm-install) | `i` | 从远程安装 SDK 版本 |
| [`sdkm list`](#sdkm-list) | `ls`, `l` | 列出本地/远程版本（含交互式 TUI） |
| [`sdkm switch`](#sdkm-switch) | `s` | 切换到本地已安装版本（全局） |
| [`sdkm use`](#sdkm-use) | — | 为当前项目 / 当前 shell 会话固定版本 |
| [`sdkm env`](#sdkm-env) | — | 输出当前目录应 eval 的环境脚本（hook 内部用） |
| [`sdkm hook`](#sdkm-hook) | — | 输出 shell hook 注册脚本（注入 profile 用） |
| [`sdkm current`](#sdkm-current) | `c` | 查看当前激活版本 |
| [`sdkm config`](#sdkm-config) | — | 配置管理（7 个子命令） |
| [`sdkm self`](#sdkm-self) | — | 管理 sdkm 自身（卸载 / 自更新） |

---

## sdkm init

首次使用前初始化 sdkm：创建 `store/`、符号链接目录、`config.toml`，并把符号链接目录注册到 PATH。

```bash
sdkm init           # 标准初始化（检测目录部署是否合理）
sdkm init --force   # 强制重新初始化：覆盖现有 config.toml，跳过目录检测
```

- `--force` / `-f`：强制重新初始化。会覆盖已有 `config.toml`，并跳过「目录部署检测」。
- 初始化流程会透明地逐步打印每一步操作及其用途，便于排查。
- Windows 下写入系统 PATH、环境变量与创建符号链接需**管理员权限**。

---

## sdkm install

从远程下载并安装指定 SDK 版本。安装后默认自动切换到该版本（除非加 `--no-switch`）。

```bash
sdkm install <SDK> <VERSION> [--no-switch]
```

- `<SDK>`：SDK 名称。内置支持 `java` / `node` / `python` / `maven` / `go`，也接受 [custom-sdk.md](./custom-sdk.md) 中注册的自定义 SDK。
- `<VERSION>`：目标版本，**支持模糊匹配**。例如 `21` 会解析为最新的 `21.x`，`20.11` 会匹配到 `20.11.x`。
- `--no-switch`：仅安装，不自动切换到新版本。

示例：

```bash
sdkm install java 21            # 安装 Java 21 最新版并自动切换
sdkm i node 20.11.0             # 安装指定 Node.js 版本并自动切换
sdkm install python 3.12 --no-switch   # 安装 Python 3.12 但不切换
```

> **Maven 说明**：Maven 没有远程版本发现接口，因此 `<VERSION>` 必须是精确版本号（如 `3.9.9`），不支持模糊匹配，也不支持 `list -r`。详见 [已知限制](./usage.md#⚠️-已知限制)。

安装流程内部拆分为 12 个阶段（解析 → 本地检查 → 构建 URL → 下载 → 解压 → 校验 → 标准化 → 安装验证 → 清理 → 自动切换…），各阶段带进度展示，失败时会自动回滚已完成的步骤。

---

## sdkm list

列出本地已安装版本或远程可用版本。带 SDK 名时进入交互式 TUI 选择器。

```bash
sdkm list [SDK] [-r/--remote] [--limit N]
```

- 无 `<SDK>`：打印所有已安装 SDK 及其当前版本（非交互）。
- `<SDK>`（本地）：进入交互式本地版本选择器，按 `s` 切换版本。
- `<SDK> -r/--remote`：从远程拉取版本列表后进入交互式远程选择器，按 `i` 安装、`s` 切换。
- `--limit N`：远程列表最多显示 N 条，默认 20，必须 ≥ 1。
- `-r` 但未指定 `<SDK>` 会报错：`Please specify an SDK name`。
- `sdkm list maven -r` 会报错：Maven 不支持远程版本列表。

示例：

```bash
sdkm list                  # 列出所有已安装 SDK + 当前版本
sdkm ls java               # 交互式：Java 本地版本选择器
sdkm list node -r          # 交互式：Node.js 远程版本选择器
sdkm list python -r --limit 50   # 远程列表最多显示 50 条
```

### 交互式 TUI 按键

| 场景 | 按键 | 动作 |
|:---|:---|:---|
| 通用 | `↑` / `↓` 或 `k` / `j` | 上下导航 |
| 通用 | `q` / `Esc` / `Ctrl+C` | 退出 |
| 本地选择器 | `Enter` 或 `s` | 切换到选中版本 |
| 远程选择器 | `i` | 安装选中版本 |
| 远程选择器 | `s` | 切换到选中版本（需已安装） |

列表项状态标记：`✅` 当前激活 / `📦` 已安装 / 空白 = 未安装。

---

## sdkm switch

切换指定 SDK 的激活版本。目标版本必须已在本地安装。

```bash
sdkm switch <SDK> <VERSION>
```

- `<SDK>`：SDK 名称（内置或自定义）。
- `<VERSION>`：目标版本，必须是本地 `store/<sdk>/` 下已存在的版本。

示例：

```bash
sdkm switch java 21          # 切换到 Java 21
sdkm s node 20.11.0          # 切换到 Node.js 20.11.0
```

切换机制：创建/更新符号链接 `<symlink_dir>/<sdk_name>` → `store/<sdk>/<version>`，将符号链接的 bin 目录加入 PATH，并通过平台特定方式设置额外环境变量（如 `JAVA_HOME`），最后更新 `config.toml` 的 `current_version`。

**生效时机**：已注入 shell hook 时，切换完成**按一下回车**（提示符渲染触发 hook → `sdkm env` 重建 PATH）即生效，无需重启终端；未注入 hook 时需重启终端（或重新 source profile）。

**安全特性**：

- **PATH 冲突检测**：切换前检测 PATH 中是否已有同 SDK 的其他版本路径，避免冲突。
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

- 在当前目录生成（或合并进已有的）`.sdkm.toml`，把 SDK → 期望版本作为 KV 摊平写入（如 `java = "21"`）。
- **声明意图，不强制安装**：若该版本本地未装，仍写入，但 `sdkm env` 会自动降级回全局版本，并提示你 `sdkm install java 21`。
- 写入前若检测到上层目录已有 `.sdkm.toml`，会 warning 提示覆盖关系（只提示不阻断）。
- 写入后**按一下回车**（hook 触发 `sdkm env` 重读配置）即激活，无需重启终端或重新 cd。

### 会话级（`--shell`）

```bash
eval "$(sdkm use --shell java 21)"          # bash / zsh
sdkm use --shell java 21 | source            # fish
Invoke-Expression ((sdkm use --shell java 21) -join [Environment]::NewLine)  # PowerShell
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
- 输出内容：PATH 重建行（四层 bins 前置到 `_SDKM_BASE_PATH`：临时/项目 = store 真实版本目录，全局 = symlink 目录）+ 全局 known env vars 的幂等 unset + 本次选中 SDK 的 export（如 `JAVA_HOME`）。
- **幂等重建**：每次调用都从 base PATH 重建，离开项目目录即还原全局；无项目配置时输出幂等还原脚本。
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

查看当前激活的 SDK 版本。

```bash
sdkm current [SDK]
```

- 无 `<SDK>`：显示所有 SDK 的当前激活版本。
- `<SDK>`：仅显示指定 SDK 的当前版本。

示例：

```bash
sdkm current         # 显示所有 SDK 当前版本
sdkm c java          # 仅显示 Java 当前版本
```

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

**内置 SDK 保护**：内置 SDK（java/node/python/maven/go）的所有字段不可 `delete`，也不可 `remove-sdk`，只能通过 `set` 修改。

---

## sdkm self

管理 sdkm 自身，含两个子命令。

### sdkm self uninstall

卸载 sdkm：清理所有被管理 SDK 的激活环境（符号链接 / PATH / 环境变量 / `current_version`），并删除 home 目录内容（store/links/.tmp/cache/config.toml）。破坏性操作，强制交互确认、不可跳过。sdkm 二进制本身与 PATH 条目需手动清理（运行中的 exe 跨平台不可靠自删）。

### sdkm self update

从 GitHub Release 检查最新版并就地替换 sdkm 二进制，带备份 + 验证 + 自动回滚。

```bash
sdkm self update              # 检查 GitHub 最新版，落后则下载替换（只升不降）
sdkm self update --check      # 只打印 current vs latest，不下载
sdkm self update --rollback   # 回滚到上次更新前的备份
sdkm self u -c                 # 别名 u + 短 flag -c/-r
```

- 别名 `u`；`--check`/`-c`、`--rollback`/`-r`，二者互斥。
- **只升不降**：远程 ≤ 当前 → 提示已是最新、不下载。
- **备份与回滚**：替换前备份当前二进制到 `<home>/.tmp/self_update/`，替换后 spawn `--version` 验证；失败自动回滚到旧版。`--rollback` 从该备份恢复；备份不存在则拒绝回滚。
- 复用 `config.toml` 的网络设置（`network.proxy` / `ssl_verify` / `github_token` / `connect_timeout` 自动生效）。
- 产物名格式 `sdkm-<platform>.<ext>`（如 `sdkm-windows-x86_64.zip`），平台在编译期匹配。
- Windows 上替换运行中的二进制靠 rename（非 delete）；旧副本残留在 `.tmp/self_update/`，下次运行清理，不污染 sdkm 安装目录。
