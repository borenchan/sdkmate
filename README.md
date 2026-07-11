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

| 能力 | sdkm | sdkman | nvm / pyenv / jenv |
|:---|:---:|:---:|:---:|
| 多语言统一管理 | ✅ Java/Node/Python/Maven + 自定义 | ⚠️ 以 Java 生态为主 | ❌ 一个工具只管一种语言 |
| Windows 原生支持 | ✅ 一等公民，注册表 + 广播 | ❌ 需 WSL | ⚠️ 需第三方移植版 |
| 已开进程感知切换 | ✅ Windows 广播通知已开程序 | ❌ 仅当前 shell | ❌ 仅当前 shell |
| 切换默认全局持久 | ✅ 符号链接 + 系统 PATH 一次到位 | ⚠️ `sdk use` 临时，需 `default` 持久 | ⚠️ `use`/`shell` 临时，需额外命令持久 |
| 模糊版本匹配 | ✅ `21` → 最新 21.x + 相近建议 | ❌ 不支持前缀模糊 | ⚠️ 部分支持 |
| 单文件可移植 | ✅ 二进制 + 配置同目录 | ❌ 脚本 + 固定安装路径 | ❌ 脚本 + shell 钩子 |
| 操作可回滚 | ✅ 快照自动恢复 | ❌ | ❌ |
| 内存安全实现 | ⚡ Rust 所有权 + 编译期检查 | 🐌 无类型检查的 bash | 🐌 无类型检查的 shell |
| AI agent 友好 | ✅ 退出码语义 + 全局生效 + skill 文档 | ⚠️ shell 函数，跨进程受限 | ⚠️ 切换仅当前 shell，跨进程受限 |
| 实现 | ⚡ Rust 编译二进制 | 🐌 bash 脚本 | 🐌 bash / shell 脚本 |

</div>

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
sdkm init          # 首次使用：创建目录结构、注册环境变量
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
