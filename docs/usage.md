# 详细用法

> 读者：普通用户。

本节是 sdkm 的完整使用文档入口。如果你只想要「30 秒跑起来」，看 [项目 README 的 Quick Start](../README.md) 即可。

## 文档导航

**使用者文档**

| 文档 | 内容 |
|:---|:---|
| [commands.md](./commands.md) | 每个子命令的参数、别名、行为与示例（init / install / uninstall / list / switch / use / env / hook / current / config / doctor / clean / self） |
| [configuration.md](./configuration.md) | `config.toml` 结构、每个配置项含义、类型校验规则、写入安全机制 |
| [custom-sdk.md](./custom-sdk.md) | 用 `add-sdk` 注册任意工具为自定义 SDK、URL 模板占位符系统、42 个内置 SDK 名单 |
| [sdk-onboarding.md](./sdk-onboarding.md) | 什么工具适合接入 sdkm、四类支持范围、用户自助注册 vs 贡献内置两条路径 |
| [gui.md](./gui.md) | 桌面应用 `sdkm-gui`：首次引导、五个页面、更新与重启 |
| [faq.md](./faq.md) | 常见问题排查，以 `sdkm doctor` 为核心 |

**贡献者文档**

| 文档 | 内容 |
|:---|:---|
| [development.md](./development.md) | 构建、分层架构、测试沙箱、质量门禁、CLI 命令落位 |
| [development-notes.md](./development-notes.md) | 易变开发注意事项：历史坑、跨层契约与平台约束 |

本文档章节索引：

- [项目级版本管理](#项目级版本管理) — `.sdkm.toml` + `sdkm use`，版本随目录自动切换
- [Shell 支持与 Hook 注册](#shell-支持与-hook-注册) — 4 种 shell 支持与手动注入 / 重置
- [交互式 TUI](#交互式-tui) — 两层浏览：SDK 选择器 + 版本选择器
- [自定义 SDK](#自定义-sdk) — 把任意工具纳入 sdkm 管理
- [桌面应用（GUI）](#桌面应用gui) — `sdkm-gui` 页面导览与使用入口
- [工作原理](#工作原理) — hook 生效流程图、四层优先级、关键机制（技术参考）

## 30 秒快速上手

```bash
# 1. 初始化（首次使用）
sdkm init

# 2. 安装一个 SDK（支持模糊匹配，自动切换）
sdkm install java 21

# 3. 或者把已有的本地 SDK 交给 sdkm 托管：
#    放到 <sdkm所在目录>/store/java/21/ 下，sdkm 会自动发现

# 4. 列出与切换
sdkm list                  # TUI 浏览所有 SDK 与版本（非 TTY 下打印已安装概览）
sdkm list node -r          # 交互式浏览远程 Node.js 版本，按 i 安装
sdkm switch java 17        # 切换到本地已安装的 Java 17
sdkm current               # 查看当前生效版本及其来源
```

## 把已有 SDK 交给 sdkm 托管

sdkm 不强制从远程安装。你可以把已经装好的 SDK 直接放进 `store/` 目录，sdkm 会自动发现并纳入管理：

```
<sdkm所在目录>/store/java/21/      # JDK 21
<sdkm所在目录>/store/node/22/      # Node.js 22
<sdkm所在目录>/store/python/3.12/  # Python 3.12
```

放进目录后 `sdkm list java` 即可看到，`sdkm switch java 21` 即可切换。**无需手动配置环境变量、无需重启终端**——切换后通过符号链接 + PATH 注入实时生效。

## 项目级版本管理

不同项目用不同版本是全栈工程师的常态：A 项目 Java 21、B 项目还在 Java 17。`sdkm use` 让版本**随目录自动切换**——进目录用项目版本，离开还原全局，改完 `.sdkm.toml` 按回车即热更新，不用重开终端。

> **推荐用项目级（`sdkm use`，默认）而非临时级（`--shell`）**。项目级把版本写进 `.sdkm.toml`，持久留存、可提交版本库与团队共享、`cd` 进出该目录自动切；临时级只对当前终端生效，关掉终端即失效、换一个终端又要重来，也不便复现。临时级更适合「临时覆盖某个项目版本跑一次测试」这类一次性场景。

在项目根目录放一个 `.sdkm.toml`，摊平写 SDK → 版本：

```toml
java = "21"
node = "20.11.0"
```

用法：

```bash
# 项目级：在当前目录写 .sdkm.toml（默认）
sdkm use java 21              # '21' 模糊匹配本地已装的最新 21.x
sdkm use node 20.11.0

# 临时级：只对当前终端生效，不写文件（优先级最高，覆盖项目与全局）
eval "$(sdkm use --shell java 21)"     # bash / zsh
sdkm use --shell java 21 | source      # fish
sdkm use --shell java 21 | iex         # PowerShell
```

几点须知：

- **先注入 hook**：项目级 / 临时级靠 shell hook 自动生效，未注入时只有 `switch`（全局）有效。先跑 `sdkm init`（见下节 [Shell 支持与 Hook 注册](#shell-支持与-hook-注册)）。
- **写入要求本地已装**：`sdkm use java 21` 要求本地有可模糊匹配的已装版本（写入解析后的精确版本）；未装或匹配失败会报错拒绝写入，先 `sdkm install` 再 pin。手写 `.sdkm.toml` 未装版本时 `sdkm env` 自动回退全局并提示安装。
- **建议提交版本库**：团队 clone 后 `sdkm init` 即自动用对版本（前提是各自本地已装）。

> 三种作用域如何排序、hook 如何读 `.sdkm.toml`、未装降级与离开目录自动还原的内部机制，见 [工作原理](#工作原理)。

---

## Shell 支持与 Hook 注册

项目级 / 临时级版本切换依赖 shell hook。sdkm 支持 4 种 shell，`sdkm init` 会自动检测当前 shell 并把 hook 注入到对应 profile。

**检测方式**（Unix）：按登录 shell（`$SHELL`）判定，未知兜底 bash。注意 `$SHELL` 不会随手动启动的 shell 改变——例如登录 shell 是 bash、再手动敲 `fish` 进入，sdkm 仍会按 bash 处理。非登录 shell 用户请在该 shell 的 profile 里手动注入 hook（见下文「手动注册 hook」），或直接把该 shell 设为登录 shell。

| Shell | profile 路径 |
|:---|:---|
| **bash** | `~/.bashrc` |
| **zsh** | `~/.zshrc` |
| **fish** | `~/.config/fish/config.fish` |
| **PowerShell** | `Documents\PowerShell\Microsoft.PowerShell_profile.ps1`（PS7）+ `Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`（PS5.1） |

> PowerShell 会同时注入 PS7 + PS5.1 两个 profile（用户日常可能用任一版本）。注入后 **重启 shell** 生效。日常无需手动操作，以下仅在你需要手动注入、重置或排查时参考。各 shell 的 hook 触发机制与 PATH 持久化方式见 [工作原理](#工作原理)。

### 自动注入（推荐）

```bash
sdkm init            # 自动检测登录 shell（$SHELL）并注入 hook
```

重复跑 `sdkm init` 安全——已注入会跳过（去重），不会重复追加。

**环境变量不再直接写进 profile**（Unix）：`init`/`switch` 把 PATH 与 `JAVA_HOME` 等写入托管环境文件 `<sdkm_home>/sdkm_env.sh`（fish 为 `sdkm_env.fish`），profile 只保留一行带注释的 source：

```sh
# sdkm environment (managed by sdkm, do not edit)
[ -f "/path/to/sdkm/sdkm_env.sh" ] && . "/path/to/sdkm/sdkm_env.sh"
```

想按职责拆分文件（如统一放进 `~/.devrc`）可指定自定义路径，sdkm 只改标记块内的内容：

```bash
sdkm config set env_file "~/.devrc"   # 内容自动迁移，profile 的 source 行自动重定向
sdkm config set env_file ""           # 恢复默认（<sdkm_home>/sdkm_env.sh|fish），迁移与重定向同上；等价 `sdkm config delete env_file`
```

> 从旧版本升级：日常使用中 profile 里旧版写入的 `export PATH=...` 行**不会**主动清理（它们指向稳定的 symlink 目录，换版本后依然正确；PATH 重复条目会被 hook 去重收敛）。执行 `sdkm self uninstall` 时会统一精准清扫：只删路径位于 sdkm home / `links/` 目录树内的行，用户自己的内容不动。Windows 不受影响（环境变量写注册表）。

### 手动注册 hook

若 `init` 没注入成功（如 profile 路径非标准、权限问题），或你想注入到非当前 shell，可手动把 `sdkm hook` 的输出加进 profile：

#### bash

把这行加进 `~/.bashrc`：

```bash
eval "$(sdkm hook bash)"
```

#### zsh

把这行加进 `~/.zshrc`：

```zsh
eval "$(sdkm hook zsh)"
```

#### fish

把这行加进 `~/.config/fish/config.fish`：

```fish
sdkm hook fish | source
```

> fish **必须用 `| source`**，不能用 `eval (sdkm hook fish)`——fish 的命令替换按换行拆参、eval 空格连接会把多行脚本压成一行而破坏语法。

#### PowerShell

把这行加进你的 `$PROFILE`（PS7 是 `Documents\PowerShell\Microsoft.PowerShell_profile.ps1`，PS5.1 是 `Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`）：

```powershell
Invoke-Expression ((sdkm hook powershell) -join [Environment]::NewLine)
```

> 必须用 `-join [Environment]::NewLine` 拼回单字符串再 IEX；PS 5.1 把原生命令输出按行捕获成数组，直接 IEX 会报「意外标记 }」。脚本须 ASCII（中文系统 GBK 代码页下非 ASCII 会破坏引号配对），`sdkm hook` 输出已保证 ASCII。

保存后重启 shell（或 `source ~/.bashrc` / `source ~/.zshrc` / 重开 fish / `. $PROFILE`）即生效。

### 查看 hook 脚本内容

不修改任何文件，只打印脚本：

```bash
sdkm hook bash           # 看 bash hook 脚本
sdkm hook fish
sdkm hook powershell
sdkm env --shell bash    # 看当前目录会 eval 的环境脚本
```

### 重置 / 卸载 hook

`init` 注入的 hook 在 profile 里以注释块标记，整块删除即可：

```bash
# profile 里的形态（bash/zsh 示例）：

# sdkm project-level version hook
eval "$(sdkm hook bash)"
```

手动删掉这一行（fish 删 `sdkm hook fish | source`、PowerShell 删那一整行 `Invoke-Expression ...`）即可卸载 hook。删后 `sdkm switch`（全局符号链接）仍正常工作，只是项目级 / 会话级自动切换不再生效。

### Windows 重置 PowerShell profile

收到 `Shell hook already injected in your PowerShell profile` 提示、想重新验证功能时：

```powershell
# 1. 看 $PROFILE 实际路径（PS7 / PS5.1 各自一个）
$PROFILE

# 2. 用编辑器打开（或 notepad）
notepad $PROFILE

# 3. 删掉这两行（整块）：
#   # sdkm project-level version hook
#   Invoke-Expression ((sdkm hook powershell) -join [Environment]::NewLine)
# 保存后重开 PowerShell

# 4. 重新注入
sdkm init
```

> 若你把 Documents 重定向到了非默认盘（如 D 盘），sdkm 会读注册表 `User Shell Folders\Personal` 定位真实路径，不会注入错文件。

---

## 交互式 TUI

`sdkm list`（无参数）在两个层级上浏览：

- **第一层（SDK 选择器）**：列出所有已注册 SDK，已安装在上、未安装在下，显示激活版本与占用空间。`Enter` 进入本地版本选择器（未安装则进远程），`r` 直接看远程，`f` 搜索，`q` 退出。
- **第二层（版本选择器）**：`Enter`/`s` 切换、`i` 安装（远程未装）、`d`/`Delete` 卸载（已装）、`f` 搜索版本号、`q` 退出（回到第一层）。

通用按键：`↑`/`↓` 或 `k`/`j` 导航，`Esc` 先清过滤、无过滤才退出，`Ctrl+C` 退出。状态标记：`✅` 当前激活 / `📦` 已安装 / 空白 = 未安装。

非 TTY（管道、CI、agent）不会进 TUI，自动降级为文本输出（概览表 / 版本列表），此时 `--limit` 生效。完整按键表见 [commands.md · 交互式 TUI 按键](./commands.md#交互式-tui-按键)。

## 配置

`sdkm config` 系列：

```bash
sdkm config list                              # 列出全部配置
sdkm config set network.proxy http://127.0.0.1:7890   # 设置代理
sdkm config set network.cache_ttl_secs 0      # 关闭版本缓存（每次都拉最新）
sdkm config edit                              # 用编辑器直接改 config.toml
```

完整配置项含义与校验规则见 [configuration.md](./configuration.md)。

## 自定义 SDK

sdkm 内置 42 个 SDK（Java、Node.js、Python、Go、Maven、Tomcat 等运行时与工具链，加 helm、terraform、ripgrep、k9s 等 30+ 开发工具，完整名单见 [custom-sdk.md · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)）。除此之外，任何能从 URL 下载解压的工具都能注册：

```bash
sdkm config add-sdk mytool \
  --download-url "https://example.com/mytool/{version}/mytool-{version}-{os}-{arch}.{ext}" \
  --bin-dir bin
```

GitHub Releases 分发的工具甚至可以只填版本源：

```bash
sdkm config add-sdk mytool --version-url "https://api.github.com/repos/owner/repo/releases"
```

详见 [custom-sdk.md](./custom-sdk.md)。想让工具成为内置 SDK（所有人 `init` 即可用），见 [sdk-onboarding.md](./sdk-onboarding.md)。

## 桌面应用（GUI）

不喜欢记命令？sdkm 还提供原生桌面应用 `sdkm-gui`（与 CLI 同一套 Rust 核心、共享同一份 `config.toml`）。通过 `sdkm-desktop-*` 组合包安装时两个二进制同目录分布，首次启动引导一步完成初始化，之后可在图形界面里浏览 / 安装 / 切换 / 卸载 SDK、配置项目版本约束、改网络与源设置、执行环境诊断与自更新。

- 安装与启动：[README](../README.md) 的 “Desktop App” 一节
- 页面导览与操作细节：[gui.md](./gui.md)

---

## 工作原理

本节是技术参考，讲 sdkm 内部如何协作。日常使用不需要读，排查问题或想深入理解时再看。

### 整体协作

sdkm 的版本切换靠三件事协作：**符号链接**（`switch` 全局生效）+ **shell hook**（`use` 项目/临时生效、`switch` 后按回车立即生效）+ **环境变量**（临时覆盖）。

核心洞察：**任何外部命令都无法修改父 shell 的环境变量**（子进程只拿到环境副本），所以 sdkm 不试图"帮你改当前终端"，而是让 shell 自己改——hook 注入到 profile 后，每次提示符渲染都由**你的 shell** 调用 `sdkm env`、把输出脚本 `eval` 进当前 shell。`sdkm env` 输出「此刻应有的完整环境」，全部四层（见下）动态计算，因此**任何版本变更（`switch`/`use`/改 `.sdkm.toml`）后按一下回车即生效，无需重启终端**。

### hook 生效流程

每次 shell 渲染提示符时，注入的钩子会调用 `sdkm env`，按当前目录重新生成环境脚本并 `eval`：

```mermaid
flowchart TD
    A["提示符渲染"] --> B["shell 调用 sdkm hook 注册的钩子"]
    B --> C["sdkm env：四层解析当前环境"]
    C --> D{"临时变量 SDKM_ACTIVE_*<br/>已设?"}
    D -->|"是"| E["用会话版本（最高优先）"]
    D -->|"否"| F{"向上递归找到 .sdkm.toml<br/>且 pin 了该 SDK?"}
    F -->|"是"| G["读 pins，模糊匹配本地已装版本"]
    F -->|"否"| H["跳过项目层"]
    G --> I{"版本本地已装?"}
    I -->|"是"| J["store 真实版本目录前置到 PATH<br/>设置 JAVA_HOME 等"]
    I -->|"否"| K["stderr 警告，该 SDK 回退全局"]
    E --> L{"会话版本本地仍已装?"}
    L -->|"是"| E2["store 真实版本目录前置到 PATH"]
    L -->|"否"| K
    E2 --> M["拼接全局层（兜底）"]
    H --> M
    J --> M
    K --> M
    M --> N["生成环境脚本<br/>PATH 外科手术 + 幂等 unset/export"]
    N --> O["shell eval 脚本，环境生效"]
    O --> P["cd / switch / 改配置 → 下一轮提示符重新解析"]
```

### 四层优先级解析

`sdkm env` 输出的 PATH = 四段拼接（同 SDK 只取最高命中层，不会重复出现）：

| 优先级 | 作用域 | 谁设置 | 载体 | PATH 条目来源 |
|:---:|:---|:---|:---|:---|
| 1（最高） | 临时 · 当前终端 | `sdkm use --shell` | `SDKM_ACTIVE_<SDK>` 环境变量 | store 真实版本目录 |
| 2 | 项目 · 当前目录及子目录 | `sdkm use` | `.sdkm.toml` pins | store 真实版本目录（绕过 symlink） |
| 3 | 全局 · 整个系统 | `sdkm switch` | `config.toml` 的 `current_version` | symlink 目录（`<symlink_dir>/<sdk>/bin`） |
| 4（兜底） | — | — | — | 当前 PATH 其余条目（`_SDKM_BASE_PATH` 仅作空 PATH 兜底锚点） |

```mermaid
flowchart TD
    A["对每个已注册 SDK 解析版本"] --> B{"SDKM_ACTIVE_* 已设?<br/>(临时级)"}
    B -->|"是"| C["用临时环境变量的值"]
    B -->|"否"| D{"向上递归找到 .sdkm.toml 且 pin 了该 SDK?"}
    D -->|"是"| E["用项目 pin 的值"]
    D -->|"否"| F{"config.toml 里 current_version 已设?"}
    F -->|"是"| G["用全局 symlink 版本"]
    F -->|"否"| H["该 SDK 不注入（PATH 靠 base 兜底）"]
    C --> I["输出该 SDK 的环境脚本"]
    E --> I
    G --> I
    H --> I
```

**关键点**：全局层不再是「还原启动快照」——sdkm 从 `config.toml` 权威状态（哪些 SDK 有 `current_version`）确定性推导出 symlink bin 路径，动态拼进 PATH。这保证了 `sdkm switch` 后**按一下回车**（hook 触发 `sdkm env`）新版本立即生效，不用重启终端；同 SDK 的项目 pin 依旧优先于全局（`covered` 去重保证每 SDK 只命中一层）。

### 关键机制

- **外科手术重建 + base PATH**：hook 首次运行时把启动 PATH 存到 `_SDKM_BASE_PATH`（兜底锚点）；之后每次 `sdkm env` 读父 shell 实时 `$PATH` 与 `_SDKM_INJECTED_PATHS`（上次注入记录），**剥掉自己的旧注入、插入本次注入**——会话内其他 PATH 修改（venv/conda activate、手动 `export PATH`）原样保留。优先级按**分层**定位：会话/项目声明（`use --shell` / pin）恒顶最前；全局层（switch 的 symlink）恒贴启动 PATH 边界，venv 等外部前插保持在它前面；删 pin / 离开项目后声明条目剥净，外部前插自然回到全局之前。重复执行不累积、离开目录自然还原。未选中的全局 env vars 幂等 `unset`、选中的 `export`（全局层选中的 SDK 也渲染 `{sdk_dir}`=symlink 目录的 extra_vars，如 JAVA_HOME）。
- **stdout 纯净**：`sdkm env` / `sdkm hook` / `sdkm use --shell` 的 stdout 只吐脚本（供 `eval`/`source`/`IEX`），诊断信息一律走 stderr，不会污染 eval 出来的环境。
- **双指纹缓存**：`sdkm env` 按 PWD + shell 缓存生成的脚本，新鲜度判据 = `.sdkm.toml` mtime + **会话指纹**（`SDKM_ACTIVE_*` 变量集合）+ **全局指纹**（symlink_dir + 各 SDK `current_version` 集合，读自 config.toml）——任一变化即重算。这保证 `use --shell` 与 `switch` 后 hook 不会吐旧缓存脚本压过新状态。缓存读写失败静默降级为实时解析。
- **未装降级**：项目/临时级版本本地未装时，`sdkm env` 跳过该 SDK 并 stderr 警告，PATH/env 回退全局，不阻断 shell；临时级 `use --shell` 则要求必须已装，否则报错退出。
- **父级配置冲突检测**：`sdkm use` 写 `.sdkm.toml` 前向上探测父级是否已有配置，命中则 warning 提示覆盖关系（只提示不阻断）。
- **会话级无 unset 入口**：`SDKM_ACTIVE_*` 目前只有 set 路径，清除靠手敲——bash/zsh `unset SDKM_ACTIVE_JAVA`、fish `set -e SDKM_ACTIVE_JAVA`、PowerShell `Remove-Item Env:SDKM_ACTIVE_JAVA`。

### 各 shell 的 hook 触发与 PATH 持久化

| Shell | hook 触发机制 | PATH 持久化位置 |
|:---|:---|:---|
| bash | `PROMPT_COMMAND` | 托管环境文件单行重建 `export PATH=...` |
| zsh | `precmd`（`add-zsh-hook`） | 托管环境文件单行重建（同 bash） |
| fish | `fish_prompt` 事件 | 托管环境文件逐行 `fish_add_path --path` |
| PowerShell | 包装 `prompt` 函数 | 走注册表（Windows，无环境文件） |

> **为什么托管环境文件里的 PATH 行不会与 hook 冲突**：hook 每次回车重建的 PATH（全局层 symlink bins + base）覆盖了持久化条目的语义——`switch` 维护 `config.toml` 的 `current_version` 与环境文件的 PATH 行一致，两者指向相同的 symlink 目录。环境文件只在「hook 未注入 / 非交互 shell / 首次启动」时兜底。profile 中的 source 行由 `init` 注入，插在 hook 行之前（hook 启动时要能调用 `sdkm env`）。

---

<a id="known-limitations"></a>

## ⚠️ 已知限制

坦诚面对当前状态，以下是使用时的注意事项：

- **Maven 版本发现依赖 Maven Central 元数据**：`sdkm install maven <version>` 支持精确版本（如 `3.9.9`）与模糊主版本（如 `3`、`3.9` → 最新稳定版，预发布被过滤）；`sdkm list maven -r` 可用。自定义 SDK 不填 `--version-url` 时仍只能精确版本安装。
- **Windows 需管理员权限**：Windows 下环境变量与系统 PATH 写入 `HKEY_LOCAL_MACHINE`、创建符号链接需管理员权限，运行 `init`/`switch` 需管理员。
- **Python 远程列表**：主源（uv metadata）完整；备源（GitHub API）受 `per_page=100` 限制，仅返回最近 100 个 release，主源正常时不触发。
- **Java macOS aarch64 无 jdk8 包**：Adoptium 不提供 jdk8 的 macOS aarch64 构建（jdk8 在 macOS 仅 x64），Apple Silicon 上 `sdkm install java 8` 会报错，改用 `17`/`21` 等支持 aarch64 的版本。
- **Java 供应商覆盖范围**：首批 4 家（Temurin 默认 / Zulu / Corretto / Liberica），Oracle、Microsoft、Semeru、GraalVM 等暂未接入。`sdkm list java -r` 只列 LTS 系列 + 最新 feature 系列的 patch；未列出的大版本（如 `24`）仍可直接 `sdkm install java 24`（解析时按需查对应供应商 API）。Corretto 无 Windows aarch64 构建，该平台会明确报错。
- **Java 裸大版本的下载源**：未配置 `network.github_download_mirror` 时，`install java 21`（默认供应商 + 裸大版本）走 `sdk.java.download_url` 模板（可换 Adoptium 镜像）；配置了镜像时改用解析到的 GitHub 直链，以便走镜像；Temurin 资产还会自动尝试清华 Adoptium 镜像（仅每个 feature 最新 patch）。精确版本与指定供应商始终使用各家 API 返回的直链（GitHub 托管的直链同样受镜像影响）。
- **Tomcat 不写 PATH**：Tomcat 的 `bin/` 里有 `startup`、`version`、`digest` 这类通用名脚本，入 PATH 容易撞名。因此内置条目设了 `add_to_path = false`，切换只更新 `CATALINA_HOME`，用 `$CATALINA_HOME/bin/...`（Windows `%CATALINA_HOME%\bin\catalina.bat`）启动。想改成入 PATH：`sdkm config set sdk.tomcat.add_to_path true`。
- **Tomcat 版本源是 HTML 目录**：版本列表主源 `archive.apache.org`（全历史但较慢，冷启动 4 个请求，之后按 `cache_ttl_secs` 缓存）；下载主源 dlcdn（官方 CDN），历史版本 dlcdn 404 时立即（不重试）回退内置华为云镜像（全历史）；首响应 8s / 静默 15s / 低速 60s 判卡死并自动切源。想直接用华为云作主源：
  ```bash
  sdkm config set sdk.tomcat.download_url "https://mirrors.huaweicloud.com/apache/tomcat/tomcat-{feature_version}/v{version}/bin/apache-tomcat-{version}.{ext}"
  ```
  其他可选：`https://mirrors.aliyun.com/apache/tomcat/...`、`https://mirrors.tuna.tsinghua.edu.cn/apache/tomcat/...`（占位符 `{feature_version}` = 主版本 9/10/11，`{ext}` = Windows `zip` / Unix `tar.gz`）。
