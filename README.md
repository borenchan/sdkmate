<div align="center">

  <h1 align="center">
    <img src="./assets/logo.png" alt="sdkm" width="200"/>
  </h1>

  <p>
    <a href="https://github.com/borenchan/sdkmate/stargazers"><img src="https://img.shields.io/github/stars/borenchan/sdkmate?style=social" alt="Stars"/></a>
    <a href="https://github.com/borenchan/sdkmate/network/members"><img src="https://img.shields.io/github/forks/borenchan/sdkmate?style=social" alt="Forks"/></a>
    <img src="https://img.shields.io/github/repo-size/borenchan/sdkmate?style=flat-square" alt="Size"/>
  </p>

  <p>
    <img src="https://img.shields.io/badge/Rust-1.92.0-orange.svg?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg?style=flat-square" alt="Platform">
    <img src="https://img.shields.io/badge/License-Apache--2.0-green.svg?style=flat-square" alt="License">
  </p>

  <h2>⚡ 专为全栈工程师打造的跨平台 SDK 版本管理器</h2>
  <p>
    <a href="./README-en.md">English</a> ·
    <strong>中文</strong>
  </p>

  <p>
    <a href="#快速开始">快速开始</a> ·
    <a href="#核心优势">核心优势</a> ·
    <a href="./docs/usage.md">详细用法</a>
  </p>
</div>

---

## 🎯 一句话简介

> **sdkm** 是一款专为全栈工程师打造的跨平台 SDK 版本管理器，一键安装与切换 Java、Node.js、Python、Maven 等开发环境，**比 sdkman/nvm/jenv/pyenv 更快、更安全、更省心**。

```bash
sdkm init && sdkm install java 21   # 初始化 + 安装 Java 21 并自动切换，一行搞定
```

---

## ✨ 核心优势

> **一个工具，替代 nvm + jenv + pyenv + sdkman**——而且更快、更安全、更省心、更跨平台。

<div align="center">
  <table>
    <tr>
      <td width="25%" align="center">
        <h3>🟢 纯绿色 · 轻量</h3>
        <p>单二进制 · 零运行时依赖 · <strong>~4MB</strong><br>拷到 U 盘即用 · 已有 SDK 放进目录即托管</p>
      </td>
      <td width="25%" align="center">
        <h3>⚡ 即时切换 · 全局生效</h3>
        <p>符号链接 + PATH + 广播<br>已开进程也感知 · 一次切换全局持久</p>
      </td>
      <td width="25%" align="center">
        <h3>🛡️ 透明可回滚</h3>
        <p>每步打印 · 任一步失败自动恢复<br>模糊匹配 21→21.x · 相近版本建议</p>
      </td>
      <td width="25%" align="center">
        <h3>🤖 AI Agent 友好</h3>
        <p>自带 skill 文档 · 退出码语义清晰<br>全局生效 + 失败回滚 · 放心调用</p>
      </td>
    </tr>
  </table>
</div>

### 🤖 让AI Agent替你管理 SDK
> 2026年了，嫌弃cli命令太多不好记？怕有学习成本？那就让Ai Agent来帮忙!

sdkm 自带一份自包含的 [agent skill 文档](./skills/SKILL.md)——Claude Code、Codex 等 AI 编程助手读一遍就能在你的机器上替你装/切 SDK：它会用 `sdkm install` / `sdkm switch` 直接操作、凭退出码判断成败、失败自动回滚，不卡在交互式提示、不踩 shell 函数的坑。

**安装 skill，任选一种即可(以Claude Code为例)：**

- **最简单 · 让 agent 自己装**：在 Claude Code 对话里直接说——

  ```
  帮我安装一个 skill: https://github.com/borenchan/sdkmate/blob/master/skills/SKILL.md
  ```

- **手动复制**：把 `SKILL.md` 放到 `~/.claude/skills/sdkm/SKILL.md`（Windows 为 `%USERPROFILE%\.claude\skills\sdkm\SKILL.md`）即可，全局可用。

装好后**重启 Claude Code 会话**（让它扫描到新 skill），然后直接对 agent 说人话，它会自动触发 sdkm skill 并执行：

```
帮我装个 java 21            → sdkm install java 21
切到 node 20.11.0          → sdkm switch node 20.11.0
看看本地装了哪些 python     → sdkm list
```

### 📊 与同类工具对比

<div align="center">

| 核心维度 | **sdkm** | **mise** | **sdkman** | **jvms / jenv / nvm / pyenv** |
| :--- | :---: | :--- | :--- | :--- |
| **多语言统一管理** | ✅ Java/Node/Python/Maven + 自定义 SDK | ✅ node、python、cmake、terraform 等 [hundreds more](https://github.com/jdx/mise?tab=readme-ov-file#what-does-it-do) | ⚠️ 以 Java/JVM 生态为主 | ❌ 单一语言管理器，一个工具只管一种语言（如 [nvm](https://github.com/nvm-sh/nvm?tab=readme-ov-file#node-version-manager---)、[pyenv](https://github.com/pyenv/pyenv?tab=readme-ov-file#simple-python-version-management)） |
| **Windows 原生支持** | ✅ **一等公民**：原生注册表 + 广播通知 | ⚠️ **基础支持**：Windows 下只能用 core/vfox 插件，[可用工具有限](https://mise.jdx.dev/windows.html#windows-support) | ❌ 官方只支持 WSL / Git Bash / Cygwin，[安装文档](https://sdkman.io/install) | ⚠️ Windows 需第三方移植版（如 [nvm-windows](https://github.com/coreybutler/nvm-windows)、[pyenv-win](https://github.com/pyenv-win/pyenv-win)） |
| **已开进程感知切换** | ✅ Windows 上通过 `WM_SETTINGCHANGE` 广播通知已开程序 | ❌ 已开进程不会重新读取 PATH；shims 也不会触发广播 | ❌ 仅当前 Shell 生效，不影响已开进程 | ❌ 仅当前 Shell 生效 |
| **全局切换机制** | ✅ **符号链接 + 系统 PATH** 一次修改，全局持久生效。**SDK运行时零开销** | ⚠️ 默认为 **shims 中间层**；或 PATH 激活（仅当前 shell hook）[官方说明](https://mise.jdx.dev/installing-mise.html#installation) | ⚠️ `sdk use` 仅当前 Shell；需 `sdk default` 才持久，[使用指南](https://sdkman.io/usage#sdk-use-command) | ⚠️ Shell 变量/钩子；`use`/`shell` 通常临时生效，需额外命令持久 |
| **路径透明度 (which/IDE)** | ✅ `which java` 直接指向真实 SDK 路径 | ⚠️ shims 下 `which node` 指向 shim；官方承认“打断 `which`”，需 `mise which` 才能看到真实路径，[设计说明](https://mise.jdx.dev/how-i-use-mise.html#shims-vs-path) | ✅ 路径透明 | ✅ 路径透明（但在 Shell 内部） |
| **模糊版本匹配** | ✅ `21` → 最新 21.x，并给出候选列表建议 | ✅ 支持模糊版本（如 `node = "26"`）及就近匹配 | ❌ 不支持前缀模糊；需要精确版本或手动选择 | ⚠️ 部分工具支持前缀匹配，实现不一 |
| **单文件可移植 / 绿色** | ✅ 单二进制仅**4MB**， 配置目录，可放在 U 盘或任意路径使用 | ⚠️ **100MB+**,需要 shims 目录（`~/.local/share/mise/shims` 或 `%LOCALAPPDATA%\mise\shims`），[安装说明](https://mise.jdx.dev/installing-mise.html#installation) | ❌ 脚本 + 固定安装路径 | ❌ 脚本 + Shell 钩子，依赖 Shell 环境 |
| **操作可回滚** | ✅ 快照自动恢复（项目特性） | ⚠️ 依赖外部手段（如自己备份配置） | ❌ 不提供自动回滚机制 | ❌ 不提供自动回滚机制 |
| **实现语言 / 内存安全** | ⚡ Rust 所有权 + 编译期检查 | ⚡ Rust 所有权 + 编译期检查 | 🐌 Bash / Shell 脚本 | 🐌 Shell / Bash 脚本 |
| **AI agent / 自动化友好** | ✅ 退出码语义清晰；全局生效，子进程自动感知；行为可预测 | ⚠️ 需理解 shims / PATH 模式差异；非交互环境需选择 shims 或 `mise exec`，[使用建议](https://mise.jdx.dev/how-i-use-mise.html#shims-vs-path) | ⚠️ Shell 函数，跨进程不自动生效 | ⚠️ 切换仅当前 Shell，跨进程不可见；需额外脚本适配 |

</div>

> **使用建议**

1. 诚实来讲,  sdkm 和 mise 几乎是同类工具，目前生态，功能都不如mise成熟。但sdkm专注于做好一件事："**快速、透明、无侵入的版本切换。**"

2. 另外 如果你已经习惯 mise / sdkman / nvm，并能在现有模式下顺畅工作，完全没必要换工具。
   sdkm 更适合：‘**全栈工程师  + Windows 原生 + IDE + AI Agent**’ 这一组合场景的用户。”

### 🔥 专为全栈工程师设计

全栈工程师在 Java、Node.js、Python、maven等等sdk版本 之间来回切换是常态——以往要同时装 nvm、jenv、pyenv、sdkman 多套脚本工具，各管一摊、互不通气。**sdkm 用一个 Rust 二进制把这件事做完。**

- **🟢 纯绿色，可移植**：单二进制就是全部，不装后台服务。sdkm 的「`HOME`」就是可执行文件所在目录——拷到 U 盘、另一台机器，配置和已装 SDK 一并带走。把已有 JDK / Node / Python 直接放进 `store/` 目录，sdkm 自动发现并托管。
- **⚡ 即时切换，影响全局**：符号链接 + PATH 注入 + 环境变量广播三件套切换版本，一次 `switch` 全局持久生效（符号链接、系统 PATH、`current_version` 同步更新）；Windows 下 `WM_SETTINGCHANGE` 广播，愿意响应的已开程序可感知新变量。
- **🛡️ 透明可回滚，出错不翻车**：每一步操作都逐步打印做了什么、为什么；`switch` 任一步骤失败自动恢复到切换前状态；`config` 采用原子写入 + 快照回滚，配置文件绝不会写到一半损坏。
- **🦀 Rust 驱动，类型安全更可靠**：由 Rust 编写，所有权与类型系统在编译期消除悬垂指针、缓冲区溢出、数据竞争等整类内存安全问题；相比无类型检查的 bash 脚本，多一层编译期兜底，不易因拼写或空值静默出错。配合原子写入 + 快照回滚，操作失败也不会把环境搞坏。
- **🧩 可扩展，一个工具管所有**：内置 Java / Node.js / Python / Maven / Any Sdk，任何能从 URL 下载解压的工具都能一行命令注册为自定义 SDK。配置按类型校验，改错当场报错——**告别"为每个语言学一套版本管理器"**。
- **🖥️ 跨平台原生 + 交互式 TUI**：Windows / Linux / macOS 全平台一等公民，统一命令统一体验；`sdkm list <sdk> -r` 进入交互式 TUI，方向键浏览远程版本、一键安装/切换，操作极其友好，Windows也能支持的很好！
- **🤖 AI agent 友好，让 AI 替你管环境**：自带 [agent skill 文档](./skills/SKILL.md)，Claude Code / Codex/openclaw 等 agent 读一遍即可替你装/切 SDK；CLI进程退出码语义清晰（0 成功 / 1 失败），CLI命令天然适配脚本与 CI，操作失败自动回滚——agent 放心调用。

---

## 📦 安装

### 📥 下载预编译二进制

前往 [Releases](https://github.com/borenchan/sdkmate/releases) 页面下载对应平台的压缩包，解压到任意目录即可（保留 `.sdkm/` 目录结构），进入 `.sdkm/` 目录即可开始使用。

---

## 🚀 快速开始

### 1️⃣ 初始化

```bash
.sdkm\sdkm.exe init          # windows;首次使用：创建目录结构、注册sdkm到PATH
.sdkm/sdkm init              # unix;
```

### 2️⃣ 安装或托管已有 SDK

```bash
# 从远程安装（支持模糊匹配，自动切换）
sdkm install java 21              # '21' → 最新 21.x
sdkm install node 20.11.0

# 或把已有 SDK 交给 sdkm 托管（无需重新下载）
# 放到 <sdkm所在目录>/store/java/21/ 下，sdkm 自动发现
```

### 3️⃣ 切换版本

```bash
sdkm switch java 17        # 切换到本地已安装的 Java 17
sdkm s node 20.11.0        # 切换到 Node.js 20.11.0
```

### 4️⃣ 交互式浏览

```bash
sdkm list                  # 列出所有已安装 SDK + 当前版本
sdkm list node -r          # 交互式浏览远程 Node.js 版本，按 i 安装、s 切换
sdkm current               # 查看所有 SDK 当前激活版本
```

📖 **完整命令、参数、配置项与自定义 SDK 详见 [详细用法文档](./docs/usage.md)**。

---

## 🎮 命令参考

| 命令 | 别名 | 说明 | 示例 |
|:---|:---|:---|:---|
| `sdkm init` | — | 初始化 sdkm | `sdkm init --force` |
| `sdkm install` | `i` | 安装 SDK 版本（模糊匹配，自动切换） | `sdkm install java 21` |
| `sdkm list` | `ls`, `l` | 列出/浏览 SDK 版本（含交互式 TUI） | `sdkm list node -r` |
| `sdkm switch` | `s` | 切换 SDK 版本 | `sdkm switch java 21` |
| `sdkm current` | `c` | 显示当前版本 | `sdkm current java` |
| `sdkm config` | — | 配置管理 | `sdkm config edit` |

---

## 🏗️ 支持的 SDK

| SDK | 支持状态 | 下载源 |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ 已支持 | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ 已支持 | nodejs.org |
| 🐍 **Python** | ✅ 已支持 | astral-sh python-build-standalone |
| 🧶 **Maven** | ✅ 已支持 | Apache Maven (dlcdn) |
| ⚙️ **自定义 SDK** | ✅ 可无限扩展 | 用户配置（任意 URL） |

---

## 🛠️ 技术栈

| 组件 | 技术 | 说明 |
|:---|:---|:---|
| **语言** | Rust 1.92.0 | 性能与安全兼得 |
| **CLI 解析** | clap | 优雅的命令行参数处理 |
| **异步运行时** | tokio | 高性能异步 IO |
| **HTTP 客户端** | reqwest | 跨平台 HTTP 请求 |
| **终端输出** | crossterm + indicatif | 彩色终端输出 + 进度条 |
| **配置解析** | toml (serde) | 人性化配置文件格式 |

---

## 🔧 开发指南

### 环境要求

- Rust 1.92.0（edition 2024）
- Cargo

### 本地开发

```bash
# 克隆项目
git clone https://github.com/borenchan/sdkmate.git
cd sdkmate

# 构建项目
cargo build --release

# 运行测试
cargo test

# 运行示例
./target/release/sdkm init
./target/release/sdkm list
./target/release/sdkm switch java 21
```

### 代码规范

```bash
cargo fmt                                              # 格式化代码
cargo clippy --all-targets --all-features              # 代码检查
```

---

## 🤝 贡献指南

我们欢迎任何形式的贡献！无论是提交 Bug 报告、功能建议，还是直接贡献代码，都非常感谢。

### 📋 提交 PR 指南

1. Fork 仓库并创建分支：`git checkout -b feature/your-feature-name`
2. 编写代码并确保通过所有测试：`cargo test`
3. 提交代码，使用清晰的提交信息：`git commit -m "feat: add xxx"`
4. 推送分支并创建 Pull Request：`git push origin feature/your-feature-name`

### 📖 开发约定

| 类型 | 规范 | 示例 |
|:---|:---|:---|
| Commit 消息 | `type: description` | `feat: add switch command` |
| 类型 | feat / fix / docs / refactor / test / chore | — |
| 分支命名 | `feature/xxx` / `fix/xxx` / `docs/xxx` | `feature/add-install-command` |

---

## 📄 许可证

本项目采用 [Apache-2.0](./LICENSE) 许可证开源。

---

<div align="center">

**如果这个项目对你有帮助，请点个 ⭐ 支持一下！**

Made with ❤️ by the sdkm team

</div>

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=borenchan/sdkmate&type=Date)](https://star-history.com/#borenchan/sdkmate&Date)

</div>
