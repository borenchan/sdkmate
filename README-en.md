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

  <h2>⚡ Next-Gen SDK Version Manager · Version Switching Made Effortless</h2>

  <p>
    <strong>English</strong> ·
    <a href="./README.md">中文</a>
  </p>

  <p>
    <a href="#installation">Quick Install</a> ·
    <a href="#quick-start">Quick Start</a> ·
    <a href="#command-reference">Commands</a> ·
    <a href="#development">Development</a>
  </p>
</div>

---

## 🎯 Elevator Pitch

> **sdkmate** is a blazing-fast, cross-platform SDK version manager built for full-stack engineers. Switch between Java, Node.js, Python, Maven, and Rust environments with a single command. **Faster, smarter, and easier than nvm/jenv/pyenv**.

---

## ✨ Key Advantages

<div align="center">
  <table>
    <tr>
      <td width="33%" align="center">
        <h3>🚀 Extreme Performance</h3>
        <p>Built in Rust<br>Millisecond version switching<br>< 5MB memory footprint</p>
      </td>
      <td width="33%" align="center">
        <h3>🎯 Effortless UX</h3>
        <p>One-command installation<br>Zero config, works out of box<br>Smart auto-completion</p>
      </td>
      <td width="33%" align="center">
        <h3>🌈 Native Cross-Platform</h3>
        <p>Perfect Windows support<br>Linux / macOS fully supported<br>Consistent experience</p>
      </td>
    </tr>
  </table>
</div>

### 🔥 Why sdkmate is Better Than the Competition?

| Feature | sdkmate ✨ | nvm / jenv / pyenv | sdkman! |
|:---:|:---:|:---:|:---:|
| **Install Speed** | ⚡ Millisecond | 🐢 Shell script loading | 🐢 Slow JVM startup |
| **Memory Usage** | 💨 < 5MB | 💨 ~20-50MB | 💨 > 100MB |
| **Windows Support** | ✅ Native perfect | ⚠️ Needs WSL | ❌ Barely supported |
| **Multi-SDK Unified** | ✅ One tool for all | ❌ Need multiple tools | ⚠️ JVM-focused |
| **Env Broadcast** | ✅ Instant effect | ❌ Need terminal restart | ❌ Need terminal restart |
| **Symlink Switching** | ✅ Atomic operation | ⚠️ Shell-dependent | ⚠️ Manual operation |

---

## 📦 Installation

### 🔧 Install via Cargo (Recommended)

```bash
cargo install sdkm
```

### 🔥 One-Line Install Script (Linux / macOS)

```bash
curl -sSL https://raw.githubusercontent.com/borenchan/sdkmate/main/install.sh | bash
```

### 🪟 Windows Installation

```powershell
# Using winget
winget install sdkm

# Or using Scoop
scoop install sdkm
```

### 📥 Download Pre-built Binaries

Visit the [Releases](https://github.com/borenchan/sdkmate/releases) page to download the compressed package for your platform, extract it, and add the `sdkm` executable to your PATH.

---

## 🚀 Quick Start

### 1️⃣ Initialize

```bash
# First-time setup
sdkm init
```

> After initialization, sdkmate automatically creates the config directory and registers environment variables.

### 2️⃣ Check Available Versions

```bash
# List all locally installed SDK versions
sdkm list

# Check available Java versions remotely
sdkm list java --source remote

# Check available Node.js versions remotely
sdkm list node --source remote
```

### 3️⃣ Switch Versions

```bash
# Switch Java version with one command
sdkm switch java 21

# Switch Node.js to specific version
sdkm switch node 20.11.0

# Switch Python version
sdkm switch python 3.12.0
```

### 4️⃣ Verify Current Version

```bash
# Verify Java
java -version

# Verify Node.js
node -v

# Verify Python
python --version
```

---

## 🎮 Command Reference

| Command | Alias | Description | Example |
|:---|:---|:---|:---|
| `sdkm init` | - | Initialize sdkmate | `sdkm init --force` |
| `sdkm install` | `i` | Install SDK version | `sdkm install java 21` |
| `sdkm list` | `ls`, `l` | List SDK versions | `sdkm list java --source remote` |
| `sdkm switch` | `s` | Switch SDK version | `sdkm switch java 21` |
| `sdkm current` | `c` | Show current version | `sdkm current java` |
| `sdkm config` | - | Configuration management | `sdkm config edit` |

### 📋 Detailed Usage

#### `sdkm init`

Initialize sdkmate environment, creating necessary directory structure and config files.

```bash
sdkm init              # Standard initialization
sdkm init --force      # Force re-initialization
```

#### `sdkm list`

Query locally installed or remotely available SDK versions.

```bash
sdkm list                              # List all locally installed SDK versions
sdkm list java                         # List local Java versions
sdkm list node --source remote         # List remote Node.js versions
sdkm list python --source local         # List local Python versions
```

#### `sdkm switch`

Switch the active version of the specified SDK.

```bash
sdkm switch <SDK> <VERSION>

# Examples
sdkm switch java 21        # Switch to Java 21
sdkm switch node 20        # Switch to Node.js 20.x latest
sdkm switch python 3.11    # Switch to Python 3.11
```

#### `sdkm install` *(Coming Soon)*

Install the specified version of an SDK.

```bash
sdkm install java 21      # Install Java 21
sdkm install node 20.11.0  # Install Node.js 20.11.0
```

#### `sdkm current` *(Coming Soon)*

Display the currently active SDK version.

```bash
sdkm current              # Show all SDK current versions
sdkm current java         # Show current Java version
```

#### `sdkm config` *(Coming Soon)*

Manage sdkmate configuration.

```bash
sdkm config show          # Show current config
sdkm config edit          # Edit config file
sdkm config add           # Add custom SDK
```

---

## 🏗️ Supported SDKs

| SDK | Status | Download Source |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ Supported | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ Supported | nodejs.org |
| 🐍 **Python** | ✅ Supported | python.org |
| 🧶 **Maven** | ✅ Supported | Apache Maven |
| 🦀 **Rust** | ✅ Supported | rustup.rs |
| ⚙️ **Custom SDK** | 🔜 Coming Soon | User-defined |

---

## 🛠️ Tech Stack

| Component | Technology | Description |
|:---|:---|:---|
| **Language** | Rust 1.80+ | Performance meets safety |
| **CLI Parsing** | clap | Elegant command-line argument handling |
| **HTTP Client** | reqwest | Cross-platform HTTP requests |
| **Terminal Output** | crossterm | Colorful terminal output |
| **Config Parsing** | toml | Human-friendly config file format |
| **Windows API** | winreg / windows-sys | Native Windows support |

---

## 🔧 Development Guide

### Environment Requirements

- Rust 1.80+
- Cargo

### Local Development

```bash
# Clone the project
git clone https://github.com/borenchan/sdkmate.git
cd sdkmate

# Build the project
cargo build --release

# Run tests
cargo test

# Run examples
./target/release/sdkm init
./target/release/sdkm list
./target/release/sdkm switch java 21
```

### Code Standards

```bash
# Format code
cargo fmt

# Run Clippy checks
cargo clippy --all-targets --all-features
```

---

## 🤝 Contributing Guide

We welcome all forms of contributions! Whether you submit bug reports, feature requests, or directly contribute code, we greatly appreciate it.

### 🐛 Reporting Issues

If you find a bug or have a feature request, please submit via [GitHub Issues](https://github.com/borenchan/sdkmate/issues). Please include:

- Clear problem description
- Reproduction steps
- Expected behavior vs actual behavior
- Environment info (OS, Rust version, etc.)

### 🔧 Development Roadmap

Features we're developing or planning:

- [ ] `sdkm install` - SDK installation command
- [ ] `sdkm current` - Show current version
- [ ] `sdkm config` - Configuration management
- [ ] Custom SDK support
- [ ] Mirror source configuration
- [ ] Plugin system
- [ ] Interactive version selector
- [ ] Fish Shell support

### 📋 Pull Request Guidelines

1. **Fork the repo** and create a branch

   ```bash
   # Create new branch based on main
   git checkout -b feature/your-feature-name
   # Or fix a bug
   git checkout -b fix/your-bug-fix
   ```

2. **Write code** and ensure all tests pass

   ```bash
   # Development & debugging
   cargo build

   # Run tests
   cargo test

   # Format code
   cargo fmt

   # Lint code
   cargo clippy --all-targets --all-features
   ```

3. **Commit** with clear messages

   ```bash
   git commit -m "feat: add xxx feature"
   git commit -m "fix: resolve xxx issue"
   ```

4. **Push** and create a Pull Request

   ```bash
   git push origin feature/your-feature-name
   ```

5. Wait for code review, we'll process it ASAP

### 📐 Code Standards

- Follow Rust official code style
- Format code with `cargo fmt`
- Check code with `cargo clippy`
- Ensure all tests pass before submitting
- Add test cases for new features
- Update relevant documentation

### 📖 Development Conventions

| Type | Convention | Example |
|:---|:---|:---|
| Commit message | `type: description` | `feat: add switch command` |
| Types | feat / fix / docs / refactor / test / chore | - |
| Branch naming | `feature/xxx` / `fix/xxx` / `docs/xxx` | `feature/add-install-command` |

### 🙏 Acknowledgments

Thanks to all contributors!

---

## 📄 License

This project is open source under [Apache-2.0](./LICENSE) license.

---

## 🙏 Acknowledgments

Thanks to these open source projects:

- [Rust](https://www.rust-lang.org/) - Systems programming language
- [clap](https://github.com/clap-rs/clap) - Command-line argument parsing
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime

---

<div align="center">

**If this project is helpful to you, please give it a ⭐ !**

Made with ❤️ by the sdkmate team

</div>
