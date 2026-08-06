# 命令详解

本文档详述 `sdkm` 的全部子命令：参数、别名、行为与示例。快速上手见 [usage.md](./usage.md)，配置项含义见 [configuration.md](./configuration.md)，自定义 SDK 见 [custom-sdk.md](./custom-sdk.md)。

命令总览：

| 命令 | 别名 | 作用 |
|:---|:---|:---|
| [`sdkm init`](#sdkm-init) | — | 初始化 sdkm 运行环境 |
| [`sdkm install`](#sdkm-install) | `i` | 从远程安装 SDK 版本 |
| [`sdkm list`](#sdkm-list) | `ls`, `l` | 列出本地/远程版本（含交互式 TUI） |
| [`sdkm switch`](#sdkm-switch) | `s` | 切换到本地已安装版本 |
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

**安全特性**：

- **PATH 冲突检测**：切换前检测 PATH 中是否已有同 SDK 的其他版本路径，避免冲突。
- **快照回滚**：切换过程中任一步骤失败（符号链接、环境变量、PATH、配置写入），都会自动回滚到切换前的状态——恢复旧符号链接目标、旧环境变量值、移除已添加的 PATH 条目、恢复旧配置。
- Windows 下创建符号链接与写入环境变量需**管理员权限**。

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
