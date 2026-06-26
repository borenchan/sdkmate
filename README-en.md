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

  <h2>⚡ A cross-platform SDK version manager built for full-stack developers</h2>

  <p>
    <strong>English</strong> ·
    <a href="./README.md">中文</a>
  </p>

  <p>
    <a href="#quick-start">Quick Start</a> ·
    <a href="#why-sdkmate">Why sdkmate</a> ·
    <a href="./docs/usage.md">Detailed Usage</a>
  </p>
</div>

---

## 🎯 In One Line

> **sdkmate** is a cross-platform SDK version manager built for full-stack developers. Switch between Java, Node.js, Python, Maven and more with one tool — **faster, smarter, and more hassle-free than nvm / jenv / pyenv**.

```bash
sdkm init && sdkm install java 21   # init + install Java 21 and auto-switch. One line, done.
```

---

## ✨ Core Advantages

<div align="center">
  <table>
    <tr>
      <td width="33%" align="center">
        <h3>🟢 Portable · Green</h3>
        <p>Single binary, no runtime deps<br>Copy the folder and run<br>Hand your existing SDKs to it</p>
      </td>
      <td width="33%" align="center">
        <h3>⚡ Instant · No restart</h3>
        <p>Symlink + PATH injection + broadcast<br>Millisecond switching<br>Already-open processes pick it up</p>
      </td>
      <td width="33%" align="center">
        <h3>🛡️ Transparent · Rollback-safe</h3>
        <p>Every step printed aloud<br>Snapshot rollback on failure<br>Atomic config writes</p>
      </td>
    </tr>
  </table>
</div>

### 🔥 Designed for full-stack developers

Juggling Java, Node.js, Python is the norm for a full-stack developer — sdkmate manages them all from one tool, instead of running nvm, jenv, pyenv in parallel.

- **🟢 Portable, green, non-intrusive**: single binary, no service, no registry beyond what's needed. sdkm's "home" is the folder of the executable — copy the whole folder to another machine, config and installed SDKs come with it. No forced remote downloads: drop existing JDK / Node / Python into the `store/` directory and sdkmate discovers and manages them.
- **⚡ Instant switching, no terminal restart**: via symlink + PATH injection + env-var broadcast. On Windows, `WM_SETTINGCHANGE` makes already-open processes pick up the change too.
- **🛡️ Transparent and rollback-safe**: every step printed aloud with its purpose; `switch` auto-rolls back to the previous state if any step fails; `config` uses atomic write + snapshot rollback — the config file can never be left half-written.
- **🧩 Extensible, one tool for everything**: Java / Node.js / Python / Maven built in, and any tool downloadable from a URL can be registered as a custom SDK with one command. Config values are type-validated — bad values error on the spot.
- **🖥️ Native cross-platform**: Windows / Linux / macOS are all first-class, same commands, same experience. Written in Rust: millisecond startup, low memory.

---

## 📦 Installation

### 📥 Pre-built binaries

Download the archive for your platform from the [Releases](https://github.com/borenchan/sdkmate/releases) page, unzip and place `sdkm` (`sdkm.exe` on Windows) in any working directory.

---

## 🚀 Quick Start

### 1️⃣ Initialize

```bash
sdkm init          # First-time use: create dirs, register env vars
```

### 2️⃣ Install or hand existing SDKs to sdkmate

```bash
# Install from remote (fuzzy match, auto-switch)
sdkm install java 21              # '21' → latest 21.x
sdkm install node 20.11.0

# Or hand existing SDKs to sdkmate (no re-download)
# Drop them under <sdkm-dir>/store/java/21/ — sdkm discovers them automatically
```

### 3️⃣ Switch versions

```bash
sdkm switch java 17        # Switch to locally installed Java 17
sdkm s node 20.11.0        # Switch to Node.js 20.11.0
```

### 4️⃣ Browse interactively

```bash
sdkm list                  # List all installed SDKs + current versions
sdkm list node -r          # Interactive remote Node.js picker — i to install, s to switch
sdkm current               # Show active versions of all SDKs
```

📖 **Full commands, args, config options and custom SDKs: see [Detailed Usage](./docs/usage.md)**.

---

## 🎮 Command Reference

| Command | Alias | What it does | Example |
|:---|:---|:---|:---|
| `sdkm init` | — | Initialize sdkmate | `sdkm init --force` |
| `sdkm install` | `i` | Install a version (fuzzy match, auto-switch) | `sdkm install java 21` |
| `sdkm list` | `ls`, `l` | List/browse versions (interactive TUI) | `sdkm list node -r` |
| `sdkm switch` | `s` | Switch to a locally installed version | `sdkm switch java 21` |
| `sdkm current` | `c` | Show the active version | `sdkm current java` |
| `sdkm config` | — | Configuration management | `sdkm config edit` |

> Too many flags? `sdkm list <sdk>` opens an interactive TUI — arrow keys to browse, one key to install/switch.

---

## 🏗️ Supported SDKs

| SDK | Status | Source |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ Supported | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ Supported | nodejs.org |
| 🐍 **Python** | ✅ Supported | astral-sh python-build-standalone |
| 🧶 **Maven** | ✅ Supported | Apache Maven (dlcdn) |
| ⚙️ **Custom SDK** | ✅ Extensible | User-configured (any URL) |

---

## 🛠️ Tech Stack

| Component | Tech | Description |
|:---|:---|:---|
| **Language** | Rust 1.92.0 | Performance meets safety |
| **CLI parsing** | clap | Elegant argument handling |
| **Async runtime** | tokio | High-performance async IO |
| **HTTP client** | reqwest | Cross-platform HTTP |
| **Terminal output** | crossterm + indicatif | Colored output + progress bars |
| **Config parsing** | toml (serde) | Human-friendly config format |

---

## 🔧 Development

### Requirements

- Rust 1.92.0 (edition 2024)
- Cargo

### Local development

```bash
# Clone the project
git clone https://github.com/borenchan/sdkmate.git
cd sdkmate

# Build
cargo build --release

# Run tests
cargo test

# Try it out
./target/release/sdkm init
./target/release/sdkm list
./target/release/sdkm switch java 21
```

### Code quality

```bash
cargo fmt                                              # Format code
cargo clippy --all-targets --all-features              # Lint code
```

---

## 🤝 Contributing

Contributions of any kind are welcome — bug reports, feature ideas, or code.

### 📋 PR checklist

1. Fork the repo and create a branch: `git checkout -b feature/your-feature-name`
2. Write code and make sure all tests pass: `cargo test`
3. Commit with a clear message: `git commit -m "feat: add xxx"`
4. Push and open a Pull Request: `git push origin feature/your-feature-name`

### 📖 Conventions

| Type | Convention | Example |
|:---|:---|:---|
| Commit message | `type: description` | `feat: add switch command` |
| Types | feat / fix / docs / refactor / test / chore | — |
| Branch naming | `feature/xxx` / `fix/xxx` / `docs/xxx` | `feature/add-install-command` |

---

## 📄 License

This project is open-sourced under the [Apache-2.0](./LICENSE) license.

---

<div align="center">

**If this project helps you, please give it a ⭐!**

Made with ❤️ by the sdkmate team

</div>
