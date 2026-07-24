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

> **sdkmate** is a cross-platform SDK version manager built for full-stack developers. Install and switch between Java, Node.js, Python, Maven, Go and more with one tool — **faster, safer, and more hassle-free than nvm / jenv / pyenv / sdkman**.

```bash
sdkm init && sdkm install java 21   # init + install Java 21 and auto-switch. One line, done.
```

---

## ✨ Core Advantages

> **One tool to replace nvm + jenv + pyenv + sdkman** — faster, safer, simpler, and more cross-platform.

<div align="center">
  <table>
    <tr>
      <td width="25%" align="center">
        <h3>🟢 Portable · Lightweight</h3>
        <p>Single binary · zero runtime deps · <strong>~4MB</strong><br>Copy to USB and run — drop existing SDKs in to manage</p>
      </td>
      <td width="25%" align="center">
        <h3>⚡ Instant · Global</h3>
        <p>Symlink + PATH + broadcast<br>Open processes notice · one switch, system-wide</p>
      </td>
      <td width="25%" align="center">
        <h3>🛡️ Transparent · Rollback-safe</h3>
        <p>Every step printed · auto-recover on failure<br>Fuzzy match 21→21.x · suggests closest</p>
      </td>
      <td width="25%" align="center">
        <h3>🤖 AI Agent Friendly</h3>
        <p>Bundled skill doc · clear exit codes<br>System-wide + rollback-safe · safe to call</p>
      </td>
    </tr>
  </table>
</div>

### 🤖 Let an AI Agent manage your SDKs

> It's 2026 — tired of too many CLI flags? Worried about the learning curve? Let an AI agent do it for you.

sdkm ships with a self-contained [agent skill doc](./skills/SKILL.md) — AI coding assistants like Claude Code, Codex read it once and can install/switch SDKs on your machine for you: they call `sdkm install` / `sdkm switch` directly, judge success by exit code, and auto-roll back on failure — no interactive prompts to get stuck on, no shell-function traps.

**Install the skill — either way works (Claude Code shown as an example):**

- **Simplest · let the agent install it**: in a Claude Code session, just say —

  ```
  help me install a skill: https://github.com/borenchan/sdkmate/blob/master/skills/SKILL.md
  ```

- **Manual copy**: drop `SKILL.md` at `~/.claude/skills/sdkm/SKILL.md` (`%USERPROFILE%\.claude\skills\sdkm\SKILL.md` on Windows) — globally available.

Once installed, **restart your Claude Code session** (so it picks up the new skill), then just talk to the agent in plain language — it will trigger the sdkm skill and run:

```
install java 21            → sdkm install java 21
switch to node 20.11.0     → sdkm switch node 20.11.0
what python do I have      → sdkm list
```

### 📊 Comparison with similar tools

<div align="center">

| Capability | sdkm | sdkman | nvm / pyenv / jenv |
|:---|:---:|:---:|:---:|
| Multi-language in one tool | ✅ Java/Node/Python/Maven/Go + custom | ⚠️ Java ecosystem mainly | ❌ one tool per language |
| Native Windows support | ✅ first-class, registry + broadcast | ❌ needs WSL | ⚠️ needs third-party port |
| Open processes sense the switch | ✅ Windows broadcast notifies them | ❌ current shell only | ❌ current shell only |
| Switch is global & persistent by default | ✅ symlink + system PATH, one shot | ⚠️ `sdk use` is temp, needs `default` | ⚠️ `use`/`shell` is temp, needs extra cmd |
| Fuzzy version match | ✅ `21` → latest 21.x + suggestions | ❌ no prefix fuzzy | ⚠️ partial |
| Single-file portability | ✅ binary + config in one dir | ❌ script + fixed install path | ❌ script + shell hooks |
| Operations rollback-safe | ✅ snapshot auto-recovery | ❌ | ❌ |
| Memory-safe implementation | ⚡ Rust ownership + compile-time checks | 🐌 unchecked bash | 🐌 unchecked shell |
| AI-agent friendly | ✅ exit-code semantics + global effect + skill doc | ⚠️ shell function, cross-process limited | ⚠️ switch is current-shell only, cross-process limited |
| Implementation | ⚡ Rust compiled binary | 🐌 bash script | 🐌 bash / shell script |

</div>

### 🔥 Designed for full-stack developers

Switching between Java, Node.js, Python, Go, Maven and other SDK versions is the norm for full-stack devs — you used to need nvm, jenv, pyenv, sdkman, multiple script tools, each in its own silo. **sdkm does it all with one Rust binary.**

- **🟢 Portable, green**: single binary, no background service. sdkm's `HOME` is the executable's folder — copy it to a USB stick or another machine, config and installed SDKs come along. Drop existing JDK / Node / Python into the `store/` directory and sdkm discovers and manages them.
- **⚡ Instant switching, global effect**: symlink + PATH injection + env-var broadcast. One `switch` takes effect globally and persistently (symlink, system PATH, `current_version` all updated); on Windows, `WM_SETTINGCHANGE` lets willing already-open processes pick up the new vars.
- **🛡️ Transparent and rollback-safe**: every step prints what it did and why; `switch` auto-rolls back to the pre-switch state if any step fails; `config` uses atomic write + snapshot rollback — the config file can never be left half-written.
- **🦀 Rust-driven, type-safe & reliable**: written in Rust — ownership and the type system eliminate whole classes of memory-safety bugs (dangling pointers, buffer overflows, data races) at compile time; compared to unchecked bash scripts, you get a compile-time safety net and won't silently fail on a typo or null. Paired with atomic writes + snapshot rollback, a failed operation never wrecks your environment.
- **🧩 Extensible, one tool for everything**: Java / Node.js / Python / Maven / Go / any SDK built in; any tool downloadable from a URL can be registered as a custom SDK with one command. Config values are type-validated — bad values error on the spot — **say goodbye to "a version manager for every language".**
- **🖥️ Native cross-platform + interactive TUI**: Windows / Linux / macOS all first-class, same commands, same experience; `sdkm list <sdk> -r` opens an interactive TUI — arrow keys to browse remote versions, one-key install/switch, super friendly to use, and Windows is supported just as well!
- **🤖 AI-agent friendly, let AI manage your env**: ships with an [agent skill doc](./skills/SKILL.md) — Claude Code / Codex / OpenClaw and other agents read it once and can install/switch SDKs for you; CLI exit-code semantics are clear (0 success / 1 failure), CLI commands fit scripts and CI naturally, and failed operations auto-roll back — agents can call it with confidence.

---

## 📦 Installation

### 📥 Pre-built binaries

Download the archive for your platform from the [Releases](https://github.com/borenchan/sdkmate/releases) page, extract it anywhere (preserve the `.sdkm/` directory structure), then cd into `.sdkm/` to get started.

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


---

## 🏗️ Supported SDKs

| SDK | Status | Source |
|:---|:---:|:---|
| ☕ **Java (JDK)** | ✅ Supported | Adoptium Eclipse Temurin |
| 🟢 **Node.js** | ✅ Supported | nodejs.org |
| 🐍 **Python** | ✅ Supported | astral-sh python-build-standalone |
| 🧶 **Maven** | ✅ Supported | Apache Maven (dlcdn) |
| 🔵 **Go** | ✅ Supported | go.dev |
| ⚙️ **Custom SDK** | ✅ Infinitely extensible | User-configured (any URL) |

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

Made with ❤️ by the sdkm team

</div>

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=borenchan/sdkmate&type=Date)](https://star-history.com/#borenchan/sdkmate&Date)

</div>
