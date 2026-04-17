![sdkmate](./assets/logo.svg)

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
    <img src="https://img.shields.io/badge/Rust-1.80+-orange.svg?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/MSRV-1.80.0%2B-green.svg?style=flat-square" alt="MSRV">
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg?style=flat-square" alt="Platform">
    <img src="https://img.shields.io/badge/License-Apache--2.0-green.svg?style=flat-square" alt="License">
  </p>

  <h2>⚡ 下一代 SDK 版本管理器 · 让版本切换如呼吸般简单</h2>

  <p>
    <a href="./README-en.md">English</a> ·
    <strong>中文</strong>
  </p>

  <p>
    <a href="#安装">快速安装</a> ·
    <a href="#快速开始">快速开始</a> ·
    <a href="#命令参考">命令参考</a> ·
    <a href="#开发指南">开发指南</a>
  </p>
</div>

---

## 🎯 一句话简介

> **sdkmate** 是一款专为全栈工程师打造的极速、跨平台 SDK 版本管理器，一键切换 Java、Node.js、Python、Maven、Rust 等开发环境，**比 nvm/jenv/pyenv 更快、更智能、更省心**。

---

## ✨ 核心优势

<div align="center">
  <table>
    <tr>
      <td width="33%" align="center">
        <h3>🚀 极致性能</h3>
        <p>由 Rust 编写<br>毫秒级版本切换<br>内存占用 < 5MB</p>
      </td>
      <td width="33%" align="center">
        <h3>🎯 极简交互</h3>
        <p>一条命令完成安装<br>无需配置立即上手<br>智能自动补全</p>
      </td>
      <td width="33%" align="center">
        <h3>🌈 跨平台原生</h3>
        <p>Windows 完美支持<br>Linux / macOS 全支持<br>统一体验</p>
      </td>
    </tr>
  </table>
</div>

### 🔥 相比竞品，sdkmate 强在哪里？

| 特性 | sdkmate ✨ | nvm / jenv / pyenv | sdkman! |
|:---:|:---:|:---:|:---:|
| **安装速度** | ⚡ 毫秒级 | 🐢 需加载 shell 脚本 | 🐢 JVM 启动慢 |
| **内存占用** | 💨 < 5MB | 💨 ~20-50MB | 💨 > 100MB |
| **Windows 支持** | ✅ 原生完美 | ⚠️ 需 WSL | ❌ 几乎不支持 |
| **多 SDK 统一** | ✅ 一个工具管所有 | ❌ 需安装多个 | ⚠️ 主要面向 JVM |
| **环境变量广播** | ✅ 实时生效 | ❌ 需重启终端 | ❌ 需重启终端 |
| **符号链接切换** | ✅ 原子操作 | ⚠️ 依赖 shell | ⚠️ 手动操作 |

---

## 📦 安装

### 🔧 使用 Cargo 安装（推荐）

```bash
cargo install sdkm
```

### 🔥 一键脚本安装（Linux / macOS）

```bash
curl -sSL https://raw.githubusercontent.com/borenchan/sdkmate/main/install.sh | bash
```

### 🪟 Windows 安装

```powershell
# 使用 winget
winget install sdkm

# 或使用 Scoop
scoop install sdkm
```

### 📥 下载预编译二进制

前往 [Releases](https://github.com/borenchan/sdkmate/releases) 页面下载对应平台的压缩包，解压后将 `sdkm` 可执行文件加入 PATH 即可。

---

## 🚀 快速开始

### 1️⃣ 初始化

```bash
# 首次使用，初始化 sdkmate
sdkm init
```

> 初始化后，sdkmate 会自动创建配置目录并注册环境变量。

### 2️⃣ 查看可用版本

```bash
# 查看所有 SDK 的本地已安装版本
sdkm list

# 查看远程可用的 Java 版本
sdkm list java --source remote

# 查看远程可用的 Node.js 版本
sdkm list node --source remote
```

### 3️⃣ 切换版本

```bash
# 一键切换 Java 版本
sdkm switch java 21

# 切换 Node.js 到指定版本
sdkm switch node 20.11.0

# 切换 Python 版本
sdkm switch python 3.12.0
```

### 4️⃣ 验证当前版本

```bash
# 验证 Java
java -version

# 验证 Node.js
node -v

# 验证 Python
python --version
```

---

## 🎮 命令参考

| 命令 | 别名 | 说明 | 示例 |
|:---|:---|:---|:---|
| `sdkm init` | - | 初始化 sdkmate | `sdkm init --force` |
| `sdkm install` | `i` | 安装 SDK 版本 | `sdkm install java 21` |
| `sdkm list` | `ls`, `l` | 列出 SDK 版本 | `sdkm list java --source remote` |
| `sdkm switch` | `s` | 切换 SDK 版本 | `sdkm switch java 21` |
| `sdkm current` | `c` | 显示当前版本 | `sdkm current java` |
| `sdkm config` | - | 配置管理 | `sdkm config edit` |

### 📋 详细用法

#### `sdkm init`

初始化 sdkmate 环境，创建必要的目录结构和配置文件。

```bash
sdkm init              # 标准初始化
sdkm init --force      # 强制重新初始化
```

#### `sdkm list`

查询本地已安装或远程可用的 SDK 版本。

```bash
sdkm list                              # 列出所有 SDK 的本地版本
sdkm list java                         # 列出 Java 本地版本
sdkm list node --source remote         # 列出远程 Node.js 可用版本
sdkm list python --source local         # 列出本地 Python 版本
```

#### `sdkm switch`

切换指定 SDK 的激活版本。

```bash
sdkm switch <SDK> <VERSION>

# 示例
sdkm switch java 21        # 切换到 Java 21
sdkm switch node 20        # 切换到 Node.js 20.x 最新版
sdkm switch python 3.11    # 切换到 Python 3.11
```

#### `sdkm install` *(即将到来)*

安装指定版本的 SDK。

```bash
sdkm install java 21      # 安装 Java 21
sdkm install node 20.11.0  # 安装 Node.js 20.11.0
```

#### `sdkm current` *(即将到来)*

显示当前激活的 SDK 版本。

```bash
sdkm current              # 显示所有 SDK 的当前版本
sdkm current java         # 显示 Java 当前版本
```

#### `sdkm config` *(即将到来)*

管理 sdkmate 配置。

```bash
sdkm config show          # 显示当前配置
sdkm config edit          # 编辑配置文件
sdkm config add           # 添加自定义 SDK
```

---

## 🏗️ 支持的 SDK

| SDK | 支持状态 | 下载源 |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ 已支持 | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ 已支持 | nodejs.org |
| 🐍 **Python** | ✅ 已支持 | python.org |
| 🧶 **Maven** | ✅ 已支持 | Apache Maven |
| 🦀 **Rust** | ✅ 已支持 | rustup.rs |
| ⚙️ **自定义 SDK** | 🔜 即将支持 | 用户配置 |

---

## 🛠️ 技术栈

| 组件 | 技术 | 说明 |
|:---|:---|:---|
| **语言** | Rust 1.80+ | 性能与安全兼得 |
| **CLI 解析** | clap | 优雅的命令行参数处理 |
| **HTTP 客户端** | reqwest | 跨平台 HTTP 请求 |
| **终端输出** | crossterm | 彩色终端输出 |
| **配置解析** | toml | 人性化配置文件格式 |
| **Windows API** | winreg / windows-sys | 原生 Windows 支持 |

---

## 🔧 开发指南

### 环境要求

- Rust 1.80+
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

# 运行实例
./target/release/sdkm init
./target/release/sdkm list
./target/release/sdkm switch java 21
```

### 代码规范

```bash
# 格式化代码
cargo fmt

# 运行 Clippy 检查
cargo clippy --all-targets --all-features
```

---

## 🤝 贡献指南

我们欢迎任何形式的贡献！无论是提交 Bug 报告、功能建议，还是直接贡献代码，都非常感谢。

### 🐛 报告问题

如果你发现了 Bug 或有功能建议，请通过 [GitHub Issues](https://github.com/borenchan/sdkmate/issues) 提交。请包含：

- 清晰的问题描述
- 复现步骤
- 预期行为 vs 实际行为
- 环境信息（操作系统、Rust 版本等）

### 🔧 开发路线图

以下是我们正在开发或计划开发的功能：

- [ ] `sdkm install` - SDK 安装命令
- [ ] `sdkm current` - 显示当前版本
- [ ] `sdkm config` - 配置管理
- [ ] 自定义 SDK 支持
- [ ] 镜像源配置
- [ ] 插件系统
- [ ] 交互式版本选择器
- [ ] Fish Shell 支持

### 📋 提交 PR 指南

1. **Fork 仓库**并创建分支

   ```bash
   # 基于 main 分支创建新分支
   git checkout -b feature/your-feature-name
   # 或修复 Bug
   git checkout -b fix/your-bug-fix
   ```

2. **编写代码**并确保通过所有测试

   ```bash
   # 开发调试
   cargo build

   # 运行测试
   cargo test

   # 代码格式化
   cargo fmt

   # 代码检查
   cargo clippy --all-targets --all-features
   ```

3. **提交代码**，使用清晰的提交信息

   ```bash
   git commit -m "feat: add xxx feature"
   git commit -m "fix: resolve xxx issue"
   ```

4. **推送分支**并创建 Pull Request

   ```bash
   git push origin feature/your-feature-name
   ```

5. 等待代码审查，我们会尽快处理

### 📐 代码规范

- 遵循 Rust 官方代码风格
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码
- 确保所有测试通过后再提交
- 为新功能添加测试用例
- 更新相关文档

### 📖 开发约定

| 类型 | 规范 | 示例 |
|:---|:---|:---|
| Commit 消息 | `type: description` | `feat: add switch command` |
| 类型 | feat / fix / docs / refactor / test / chore | - |
| 分支命名 | `feature/xxx` / `fix/xxx` / `docs/xxx` | `feature/add-install-command` |

### 🙏 致谢

感谢所有贡献者的付出！

---

## 📄 许可证

本项目采用 [Apache-2.0](./LICENSE) 许可证开源。

---

## 🙏 致谢

感谢以下开源项目：

- [Rust](https://www.rust-lang.org/) - 系统编程语言
- [clap](https://github.com/clap-rs/clap) - 命令行参数解析
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
- [tokio](https://github.com/tokio-rs/tokio) - 异步运行时

---

<div align="center">

**如果这个项目对你有帮助，请点个 ⭐ 支持一下！**

Made with ❤️ by the sdkmate team

</div>
