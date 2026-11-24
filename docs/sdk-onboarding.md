# SDK 接入说明

> 读者：普通用户（路径一）与贡献者（路径二）。

sdkm 是面向全栈工程师的跨平台 **SDK 与通用开发工具链版本管理器**。这里的 "SDK" 要打个引号——它不局限于语言开发包，已经支持 40+ 工具（Java、Node.js、Python、Go 等语言运行时，Maven、CMake、Terraform 等工具链，ripgrep、fzf、k9s 等命令行工具）。但**核心定位始终是：SDK 的统一安装、版本管理与切换**。

本文档面向两类读者：

- **普通用户**：手上有一款 sdkm 没内置的工具，想自己纳入管理 → 走 [自定义 SDK](#路径一普通用户自助注册)，看 [`custom-sdk.md`](./custom-sdk.md)。
- **贡献者**：想让某个工具成为所有人开箱即用的内置 SDK → 走 [贡献内置种子](#路径二贡献者新增内置种子)，动手前先读本文的接入标准。

## 一个工具能不能接入

判断标准只有一条：**sdkm 能不能"装好、切走、再装回来"**。满足以下五条即可接入：

1. **有官方或可信赖的版本发行渠道**——GitHub Releases、厂商 CDN、官方下载页都行。
2. **可以获取明确的版本号**——版本号要能被稳定解析（带 `v` 前缀、`-zulu` 后缀这类都能处理）。
3. **能通过下载压缩包、解压或其他可控方式完成安装**——不是必须走官方安装向导。
4. **安装后有稳定的命令入口**——能用 PATH 或现有链接机制调用，例如 `bin/tool`、`bin/tool.exe`。
5. **不要求 sdkm 负责复杂的服务生命周期**——启动、守护、实例编排、集群配置这些不在 sdkm 职责内。

## 支持范围：四类

| 类别 | 说明 | 内置示例 |
|:---|:---|:---|
| **语言 SDK / Runtime** | 语言本身及其标准工具链，是最核心的场景 | java、node、python、go、deno |
| **开发工具链** | 构建、打包、部署、IaC 等开发流程工具 | maven、cmake、helm、terraform、uv、bun、pnpm、golangci-lint、just、task、watchexec、helmfile、packer |
| **开发辅助 CLI** | 命令行增强工具，单二进制或带少量资源的 zip 分发 | ripgrep、fzf、lazygit、gh、k9s、claude-code、fd、bat、eza、delta、starship、lazydocker、stern、dive、grpcurl、temporal、age |
| **归档式中间件 / 服务器软件** | **选择性支持**：只托管"解压即用、自己启动"的归档版 | tomcat、caddy、traefik、etcd、consul、vault、duckdb |

完整内置名单与主命令见 [`custom-sdk.md` · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)。

## 不支持的接入场景

以下情况 sdkm 不接管，接入这类工具的 PR 会被关闭：

- **需要运行安装向导的应用程序**（图形/命令行 installer、系统级 setup 程序）——sdkm 只解压归档，不跑安装器。
- **需要注册系统服务的中间件**——本文档不覆盖"装完还要 `systemctl enable` / 注册 Windows 服务"的软件。
- **需要数据库、集群、实例编排的服务**——例如需要初始化数据目录、配置主从、跑迁移脚本的服务端产品。

> 判断技巧：如果一款工具"下载一个 zip、解压、把目录里某个可执行文件加到 PATH"就能用，它大概率能接入；如果装完还要跑配置向导或注册服务，就不适合。

---

## 路径一：普通用户自助注册

任何能在本地解压、有稳定命令入口的工具，都能用 `sdkm config add-sdk` 注册为**自定义 SDK**，之后 `install` / `switch` / `list` / `uninstall` 与内置 SDK 完全一致。

最短的一条命令（GitHub Releases 工具只需一个版本源 URL）：

```bash
sdkm config add-sdk mytool --version-url "https://api.github.com/repos/owner/repo/releases"
```

只托管本地已有目录、不远程下载时，连版本源都不填：

```bash
sdkm config add-sdk mylocal --bin-dir bin
```

接着把各版本目录放进 `store/mylocal/<version>/`（或用 GUI 的「导入本地版本」），即可切换。

字段含义、URL 模板占位符、支持哪些安装形态、仅支持本地导入的边界，全部在 [`custom-sdk.md`](./custom-sdk.md)。**先读能省很多时间**：先判断你的工具属于"模板直链 / GitHub Releases / 仅下载模板 / 本地 switch-only"四种形态中的哪一种，再抄对应示例。

## 路径二：贡献者新增内置种子

想把工具做成**所有人 `sdkm init` 后开箱可用**的内置 SDK，需要提交代码。动手前请先在 issue / Discussions 里讨论，确认符合下面两条门槛。

### 准入门槛

- **常用度**：语言运行时、数据库、中间件优先；其余工具需 star ≥ 15k，或属 mega-famous 级常用工具（太冷门没有内置价值）。
- **装完即用**：资产命名与解压布局必须满足"标准 B 引擎"——安装后主命令直接命中 PATH。
  - **除名**：zip 内可执行文件带平台名的（如 `codex-x86_64-pc-windows-msvc.exe`）、裸二进制无压缩包的（如 `jq`、`mkcert`）不纳入内置。
  - 原因：sdkm 按"主命令名"做 PATH 冲突检测与资产匹配，命名不符或没有压缩包会让流程不可靠。这类工具仍可作为**自定义 SDK** 使用（见路径一）。

### 三种接入形态

| 形态 | 适用 | 实现要点 |
|:---|:---|:---|
| **标准 A：模板直链** | 版本列表 JSON + 固定 URL 模板（如 Node.js、Go） | `version_url` 返回版本列表，`download_url` 用 `{version}`/`{os}`/`{arch}`/`{ext}` 占位符渲染 |
| **标准 B：GitHub Releases** | 绝大多数 GitHub 分发工具 | `version_url` 指向 GH releases API，引擎自动从 `assets` 挑选当前平台压缩包直链；`download_url` 留空 |
| **组合模式** | GH 有版本、但 release 不含二进制资产（如 helm、terraform） | `version_url` 用 GH 版本列表，`download_url` 用官方 CDN 模板 |

### 代码位置

内置注册表是**唯一事实来源**，`init` 时物化为 `config.toml` 的 `[[sdk]]` 条目。种子按四类拆分：

```
crates/sdkm-util/src/builtin/
├── mod.rs         # SdkSeed 类型、构造函数（gh / gh_prefixed / gh_cdn）、四类 SEEDS 汇总
├── lang.rs        # 语言 SDK / Runtime（java/node/python/go/deno）
├── toolchain.rs   # 开发工具链（maven/cmake/helm/terraform/packer/uv/bun/pnpm/golangci-lint/just/task/watchexec/helmfile）
├── cli.rs         # 开发辅助 CLI（ripgrep/fzf/lazygit/gh/k9s/claude-code/fd/bat/eza/delta/starship/lazydocker/stern/dive/grpcurl/temporal/age）
└── middleware.rs  # 归档式中间件 / 服务器软件（tomcat/caddy/traefik/etcd/consul/vault/duckdb）
```

每个分类文件末尾有一个 `SEEDS` 数组；添加标准 B SDK 只要在该文件加常量并追加到本文件数组，**不用改 `mod.rs`**：

```rust
// toolchain.rs
pub const MYTOOL: SdkSeed = SdkSeed::gh(
    "mytool",
    "https://api.github.com/repos/owner/mytool/releases",
    &["mytool"],
);

pub static SEEDS: &[&SdkSeed] = &[
    // ...已有条目
    &MYTOOL,
];
```

`mod.rs` 的 `SDK_SEEDS` 只是 `&[lang::SEEDS, toolchain::SEEDS, cli::SEEDS, middleware::SEEDS]` 的汇总，遍历时自动展开；只有新增分类才需要动它。

按接入形态选构造函数，不要手写结构体字面量：

| 构造函数 | 适用 | 默认值 |
|:---|:---|:---|
| `SdkSeed::gh(name, version_url, exes)` | 标准 B：GH release 带当前平台压缩包资产 | 资产前缀 = SDK 名、根布局、Default os/arch、进 PATH |
| `SdkSeed::gh_prefixed(name, version_url, prefix, exes)` | 标准 B 特例：资产前缀 ≠ SDK 名（`claude-code`→`claude`、`duckdb`→`duckdb_cli`） | 同上，仅资产前缀不同 |
| `SdkSeed::gh_cdn(name, version_url, download_url, exes)` | 组合模式：GH 只有版本、下载走官方 CDN（helm/terraform/packer/consul/vault） | GH 版本源、Go arch 风格、进 PATH |

`.bin("bin")` 用于解压后二进制在 `bin/` 子目录（cmake、gh）。标准 A 与专属路径 SDK（`lang.rs` 的 java/node/python/go、`middleware.rs` 的 tomcat）版本与下载语义各不相同，仍用结构体字面量。

> 资产前缀默认是 **SDK 名**而不是主命令名——GH 资产普遍以项目名命名（`ripgrep-*` 里的主命令却是 `rg`），只有资产前缀与项目名都不同才需要 `gh_prefixed`。这也避免了"资产名对不上、安装时才失败"的隐蔽错误。

### 完成后要验证

```bash
cargo build -p sdkm-cli                 # 编译
sdkm config list | grep mytool          # 确认条目已物化（建议用 SDKM_HOME 指向临时目录做沙箱验证）
sdkm install mytool <version>           # 实装一次，确认能下载、解压、切到版本、命令可用
```

内置种子新增会**自动影响所有用户的 `init`**——老版本升级后 `config.toml` 会自动补全缺失的内置条目（`ensure_builtin_sdks`），自定义 SDK 条目不受影响。所以新增种子前务必确认接入信息正确、实装验证通过。

> **文档维护**：内置名单在 [`custom-sdk.md`](./custom-sdk.md#内置-sdk-名单按类别) 按四类列出。新增内置 SDK 时请在对应类别补齐这一行（CLAUDE.md 与 CLI help 文案里的 "java, node, python, go, maven, ... (see docs)" 是固定省略号形式，不用动）。不符合上述四类的工具，请优先建议用户走路径一。

## 两条路径怎么选

- **只是自己用、或团队内部用** → 路径一，`add-sdk` 五分钟搞定，还能用 `config set` 随时改源。
- **希望所有人可用、且是通用工具** → 路径二，先讨论、再实现，按准入门槛把关。

不确定时可以在 issue 里描述工具的分发方式（下载链接形态、压缩包结构、主命令名），维护者会告诉你走哪条。