<div align="center">

  <h1 align="center">
    <img src="./assets/logo.svg" alt="sdkmate" width="200"/>
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

> **sdkm** 是一款专为全栈工程师打造的跨平台 SDK 版本管理器，一键切换 Java、Node.js、Python、Maven 等开发环境，**比 sdkman/nvm/jenv/pyenv 更快、更智能、更省心**。

```bash
sdkm init && sdkm install java 21   # 初始化 + 安装 Java 21 并自动切换，一行搞定
```

---

## ✨ 核心优势

<div align="center">
  <table>
    <tr>
      <td width="33%" align="center">
        <h3>🟢 纯绿色 · 可移植</h3>
        <p>单二进制，无运行时依赖<br>目录拷走即用<br>已有 SDK 直接托管</p>
      </td>
      <td width="33%" align="center">
        <h3>⚡ 即时切换 · 无需重启</h3>
        <p>符号链接 + PATH 注入 + 广播<br>毫秒级切换<br>已开进程也感知</p>
      </td>
      <td width="33%" align="center">
        <h3>🛡️ 透明可回滚 · 出错不翻车</h3>
        <p>每步操作逐步打印<br>快照回滚自动恢复<br>配置文件原子写入</p>
      </td>
    </tr>
  </table>
</div>

### 🔥 专为全栈工程师设计

全栈工程师在 Java、Node.js、Python 之间来回切换是常态——sdkm 用一个工具统一管理所有 SDK，无需 nvm、jenv、pyenv 多工具并行。

- **🟢 纯绿色，零侵入**：单二进制就是全部，不装服务、不写系统注册表以外的地方。sdkm 的「家」就是可执行文件所在目录——拷到 U 盘、另一台机器，配置和已装 SDK 一并带走。不强制从远程下载，把已有 JDK / Node / Python 直接放进 `store/` 目录，sdkm 自动发现并托管。
- **⚡ 即时切换，无需重启终端**：通过符号链接 + PATH 注入 + 环境变量广播三件套切换版本，Windows 下通过 `WM_SETTINGCHANGE` 广播，已打开的进程也能感知。
- **🛡️ 透明可回滚，出错不翻车**：每一步操作都逐步打印做了什么、为什么；`switch` 任一步骤失败自动恢复到切换前状态；`config` 采用原子写入 + 快照回滚，配置文件绝不会写到一半损坏。
- **🧩 可扩展，一个工具管所有**：内置 Java / Node.js / Python / Maven，任何能从 URL 下载解压的工具都能一行命令注册为自定义 SDK。配置按类型校验，改错当场报错。
- **🖥️ 跨平台原生**：Windows / Linux / macOS 全平台一等公民，统一命令统一体验。由 Rust 编写，毫秒级启动、低内存占用。

---

## 📦 安装

### 📥 下载预编译二进制

前往 [Releases](https://github.com/borenchan/sdkmate/releases) 页面下载对应平台的压缩包，解压后可将 `sdkm` 可执行文件放到任意工作目录中。

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

> 命令太多记不住？`sdkm list <sdk>` 进入交互式 TUI，方向键浏览、一键安装/切换。

---

## 🏗️ 支持的 SDK

| SDK | 支持状态 | 下载源 |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ 已支持 | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ 已支持 | nodejs.org |
| 🐍 **Python** | ✅ 已支持 | astral-sh python-build-standalone |
| 🧶 **Maven** | ✅ 已支持 | Apache Maven (dlcdn) |
| ⚙️ **自定义 SDK** | ✅ 可扩展 | 用户配置（任意 URL） |

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
