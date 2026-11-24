# 自定义 SDK

> 读者：普通用户；贡献者可参考文末的内置名单。

sdkm 内置 42 个 SDK（Java、Node.js、Python、Maven、Go、Tomcat 等运行时，加 bun/deno/uv/claude-code/gh/helm/terraform/fzf/ripgrep/k9s 等引擎型工具，完整名单见[内置 SDK 名单](#内置-sdk-名单按类别)）。除此之外，任何工具都能注册为自定义 SDK，纳入 `sdkm` 统一管理——安装、切换、环境变量、卸载与内置 SDK 完全一致。

本文档讲解：如何判断你的 SDK 属于哪种接入形态、每个字段怎么配、哪些形态支持远程安装、哪些仅支持本地导入，以及 CLI 与 GUI 两条操作路径。配置项完整含义见 [configuration.md](./configuration.md)；"什么工具适合接入 sdkm、怎么成为内置 SDK"见 [SDK 接入说明](./sdk-onboarding.md)。

## 快速判断：你的 SDK 属于哪种形态

sdkm 判断一个 SDK **能否远程安装**的唯一口径：`download_url` 或 `version_url` **至少一个非空**。两者皆空 = 本地 switch-only，只能托管已有目录（GUI 会隐藏「安装新版本」入口，改用「导入本地版本」）。

| 形态 | 需要配置 | 精确版本安装 | 模糊版本 / `list -r` | 典型例子 |
|:---|:---|:---:|:---:|:---|
| **标准 A：模板直链** | `version_url`（版本列表 JSON）+ `download_url`（占位符模板） | ✅ | ✅ | Node.js、Go、Maven |
| **标准 B：GitHub Releases** | 仅 `version_url`（指向 GH Releases API） | ✅（资产直链） | ✅ | bun、fzf、k9s |
| **仅下载模板**（无版本发现） | 仅 `download_url` | ✅ | ❌ | Maven 也可这样用 |
| **本地 switch-only** | 两者皆空（`--bin-dir` 等按需） | ❌ | ❌ | 已在本地解压好的工具 |

> **一个易踩的坑**：只填 `version_url` 且数据源**不是** GitHub Releases JSON（比如自建的扁平版本数组）时，版本列表里没有下载直链，安装会失败——这种源自建场景必须同时配置 `download_url` 模板（回到标准 A）。

## 注册自定义 SDK（CLI）

```bash
sdkm config add-sdk <NAME> \
  [--download-url <URL_TEMPLATE>] \
  [--bin-dir <DIR>] \
  [--version-url <URL>] \
  [--version-fallback-url <URL>] \
  [--download-fallback-url <URL_TEMPLATE>] \
  [--extra-var KEY=VALUE] \
  [--extra-path <PATH>]
```

| 参数 | 说明 |
|:---|:---|
| `<NAME>` | SDK 唯一名称，不可与已有 SDK 重名 |
| `--download-url` | 下载主源 URL 模板，支持 `{version}` 等占位符（见[占位符一节](#url-模板占位符)） |
| `--bin-dir` | 二进制所在子目录名；**省略 = 二进制在 SDK 根目录**（如 Node.js）。必须是简单目录名（`bin`、`Scripts`），不能含 `/` 或 `\` |
| `--version-url` | 版本发现主源 URL，需返回版本列表 JSON（[支持的格式](#版本发现version_url-支持的数据格式)）；不填则不支持模糊匹配与 `list -r` |
| `--version-fallback-url` | 版本发现备源 URL（主源失败时回退） |
| `--download-fallback-url` | 下载备源 URL 模板（下载主源失败或 GH 直链卡死时回退，国内网络建议配置） |
| `--extra-var` | 额外环境变量，`KEY=VALUE` 格式，可重复；值支持 `{sdk_dir}` 等占位符 |
| `--extra-path` | 额外 PATH 条目（相对符号链接目录），可重复，如 Windows Python 的 `Scripts` |

注册后即可像内置 SDK 一样使用：

```bash
sdkm install mytool 1.2.3
sdkm switch mytool 1.2.3
sdkm current mytool
sdkm list mytool          # 本地版本选择器
```

### 示例 1：模板直链（标准 A）

假设有个名为 `mytool` 的工具，下载地址形如 `https://example.com/mytool/1.2.3/mytool-1.2.3-linux-x64.tar.gz`，解压后二进制在 `bin/` 子目录：

```bash
sdkm config add-sdk mytool \
  --version-url "https://example.com/mytool/versions.json" \
  --download-url "https://example.com/mytool/{version}/mytool-{version}-{os}-{arch}.{ext}" \
  --bin-dir bin
```

### 示例 2：GitHub Releases 一条命令接入（标准 B）

任何通过 GitHub Releases 分发压缩包的工具，只需一条 `version_url`，**无需配置下载模板**：

```bash
sdkm config add-sdk <name> --version-url "https://api.github.com/repos/<owner>/<repo>/releases"
```

引擎自动完成（零配置）：

- **版本解析**：从 `tag_name` 剥离前缀取版本号（`bun-v1.4.2` / `v2.9.6` / `0.12.11` 等形态通吃），自动过滤 `prerelease` / `draft`
- **平台资产选择**：从 `assets` 中按「OS 词 + arch 词 + 压缩包后缀」自动识别当前平台的 zip/tar.gz 直链（windows/win32、darwin/macos、x64/x86_64/amd64、arm64/aarch64 等命名惯例内置同义词表；`baseline`/`profile` 修饰变体与 musl/gnu 按运行环境自动取舍）
- **直链下载**：选中的 `browser_download_url` 直接作为下载地址

版本列表取最近 100 个 release（GH API 单页上限）。

GH 直链下载失败时自动回落 `download_fallback_url` 模板（国内网络建议配置）：

```bash
sdkm config set sdk.<name>.download_fallback_url \
  "https://ghfast.top/https://github.com/<owner>/<repo>/releases/download/v{version}/<name>-{os}-{arch}.zip"
```

`{version}` 为剥离前缀后的版本号；`{os}`/`{arch}` 命名风格可用 `sdk.<name>.os_style` 与 `sdk.<name>.arch_style` 调整（见[字段详解](#configtoml-字段详解)）。此外全局配置 `network.github_download_mirror` 可为所有 GH 域名下载自动加镜像前缀，两者不冲突。

**适用条件**：

- 资产为 zip 或 tar.gz（裸 exe 单文件暂不支持）
- 解压后二进制位于压缩包根目录或 `bin/` 子目录（`--bin-dir` 按实际布局指定）
- 资产名前缀与 SDK 名一致（如注册名 `bun` 匹配 `bun-v1.4.2-x86_64.zip` 资产内的 `bun.exe`）；前缀不一致的工具（如 ripgrep 的资产 `ripgrep-*` 对应主命令 `rg`）无法通过自定义 SDK 匹配资产，需作为内置种子登记

### 示例 3：环境变量与额外 PATH

```bash
sdkm config add-sdk groovy \
  --version-url "https://example.com/groovy/versions.json" \
  --download-url "https://example.com/groovy/{version}/apache-groovy-{version}-bin.{ext}" \
  --bin-dir bin \
  --extra-var GROOVY_HOME="{sdk_dir}"
```

切换版本时 `GROOVY_HOME` 自动指向新的激活目录。

### 示例 4：本地 switch-only（不远程安装）

只想让 sdkm 托管一个**已经在本地的**工具目录、用 `sdkm switch` 切版本、不通过 sdkm 远程下载安装时，版本与下载源全部省略：

```bash
sdkm config add-sdk mylocal --bin-dir bin
```

注册后把各版本目录放到 `store/mylocal/<version>/`（或用 GUI 的「导入本地版本」，见[下文](#管理本地版本手动放置与-gui-导入)），即可用 `sdkm switch mylocal 1.0.0` / `sdkm current mylocal` / `sdkm list mylocal` 切换与查看。

> 这种 SDK **不能** `sdkm install`（无下载源），尝试会明确报错并提示放置目录或补 `download_url`。若日后需要远程安装，`sdkm config set sdk.mylocal.download_url <url>` 补上即可。

### `add-sdk` 之外的字段

`os_style` / `arch_style` / `add_to_path` 暂无 add-sdk 参数，注册后用 `config set` 补：

```bash
# 下载源用 mac/x64 风格命名（而非默认的 darwin/x64）
sdkm config set sdk.mytool.os_style adoptium
sdkm config set sdk.mytool.arch_style adoptium

# 只设环境变量、不把 bin 目录注入 PATH（Tomcat 型 SDK：
# 避免 startup/version 这类通用脚本名污染全局 PATH）
sdkm config set sdk.mytool.add_to_path false

# 后补环境变量（任意变量名均可新增）
sdkm config set sdk.mytool.extra_vars.GROOVY_HOME "{sdk_dir}"
```

`extra_paths` 只能按索引修改已有条目（`sdk.<name>.extra_paths.0`），追加新条目请在注册时用 `--extra-path` 带上，或走 GUI 编辑源 / `sdkm config edit`。

## config.toml 字段详解

`add-sdk` 最终写入 `config.toml` 的 `[[sdk]]` 条目。全部字段如下（也可直接 `sdkm config edit` 手工编辑）：

| 字段 | 类型 | 默认 | 说明 |
|:---|:---|:---|:---|
| `name` | string | （必填） | SDK 唯一标识，同时是 `store/<name>/`、`links/<name>` 目录名与命令行里的 `<SDK>` 参数 |
| `version_url` | string | 空 | 版本发现主源；空 = 不支持模糊版本与 `list -r` |
| `version_fallback_url` | string | 空 | 版本发现备源，主源失败时回退 |
| `download_url` | string | 空 | 下载主源模板；支持占位符。与 `version_url` 至少一个非空才可远程安装 |
| `download_fallback_url` | string | 空 | 下载备源模板。标准 B SDK 用于 GH 直链失败回退；标准 A SDK 用于主模板源失败回退 |
| `bin_dir` | string | 空（根目录） | 二进制子目录名；空 = 二进制在版本根目录。必须是单级目录名，不能含路径分隔符 |
| `extra_vars` | map | 空 | 切换时设置的环境变量表，值支持占位符（如 `JAVA_HOME = "{sdk_dir}"`） |
| `extra_paths` | array | 空 | 额外 PATH 条目（相对符号链接目录），如 Windows Python 的 `Scripts` |
| `os_style` | string | `default` | 下载模板 `{os}` 的命名风格：`default`（windows/darwin/linux）/ `short`（win/darwin/linux）/ `adoptium`（windows/mac/linux） |
| `arch_style` | string | `default` | 下载模板 `{arch}` 的命名风格：`default`（x64/arm64/x86）/ `adoptium`（x64/aarch64/x86）/ `python`（amd64/arm64/win32）/ `go`（amd64/arm64/386） |
| `add_to_path` | bool | `true` | 切换时是否把 `<links>/<sdk>/<bin_dir>` 写入 PATH；`false` = 只设 `extra_vars`（Tomcat 型：`CATALINA_HOME` 指过去即可，不注入 PATH） |
| `current_version` | string | 空 | 当前激活版本，`switch` 自动维护，一般无需手动改 |

> 内置 SDK 的字段同样允许 `config set` 修改（换镜像源、改 `bin_dir` 等），只是不可 `delete` / `remove-sdk`。

## 版本发现：`version_url` 支持的数据格式

自定义 SDK 的 `version_url` 必须返回 JSON，引擎依次自动识别三种形状：

1. **扁平字符串数组**：`["3.12.8", "3.12.7", ...]`
2. **对象数组**（含 `version` 字段）：`[{"version": "3.12.8"}, ...]`
3. **GitHub Releases 数组**（标准 B）：`[{"tag_name": "v1.2.3", "assets": [...]}, ...]`，同时产出平台资产直链

注意：

- Apache 风格的 **HTML 目录索引不支持**（只有内置 Tomcat 走 HTML 解析），自定义 SDK 指向 HTML 页会解析失败
- 形状 1/2 只有版本号、**没有下载直链**——纯版本列表必须搭配 `download_url` 模板（标准 A）；只有形状 3 可以只填 `version_url` 就远程安装
- GH 形状靠 JSON 结构识别而非 URL——本地镜像 / 代理 / 自建 GH API 兼容 server 同样生效

## URL 模板占位符

下载 URL（`download_url` / `download_fallback_url`）和环境变量值都支持占位符，在安装/切换时由 `TemplateRenderer` 自动替换。

### 通用占位符

| 占位符 | 含义 | 示例值 |
|:---|:---|:---|
| `{version}` | 完整版本号 | `21`、`v20.11.0`、`3.12.0` |
| `{os}` | 操作系统名（风格由 `os_style` 控制） | `windows` / `linux` / `darwin` |
| `{arch}` | CPU 架构（风格由 `arch_style` 控制） | `x64` / `arm64` / `x86` |
| `{ext}` | 平台压缩包扩展名 | Windows: `zip`；Linux/macOS: `tar.gz` |

### 路径类占位符（用于环境变量值）

| 占位符 | 含义 |
|:---|:---|
| `{sdk_dir}` | 当前 SDK 的符号链接目录绝对路径（即激活版本目录） |
| `{sdkm_home}` | sdkm 可执行文件所在目录（home 目录） |
| `{sdks_install_dir}` | SDK 安装根目录（`<home>/store/`） |

> 经典用法：`--extra-var JAVA_HOME="{sdk_dir}"` —— 切换版本时 `JAVA_HOME` 自动指向新的激活目录。

### SDK 专用占位符

个别内置 SDK 的下载源还需要动态发现的值，自定义 SDK 一般用不到（版本发现不产出这些字段）：

| 占位符 | 含义 | 用于 |
|:---|:---|:---|
| `{feature_version}` | 大版本号（如 `21`） | Java（Adoptium）、Tomcat（Apache 分支目录 `tomcat-11`） |
| `{release_tag}` | 构建日期标签（如 `20241216`，动态发现） | Python（python-build-standalone） |
| `{platform}` | 平台三元组（如 `x86_64-pc-windows-msvc`，自动检测） | Python |

## 管理本地版本：手动放置与 GUI 导入

无论哪种形态的 SDK，本地版本目录统一放在 `store/<sdk>/<version>/`。有两条等价路径把它变成 sdkm 托管的版本：

### 手动放置（CLI 用户）

把解压好的版本目录直接放到 `store/mylocal/1.0.0/`，`sdkm list mylocal` 即可看到并切换。目录内部布局由你保证（`bin_dir` 配置要与实际一致，否则 PATH 指不到可执行文件）。

### GUI「导入本地版本」

SDK 页每个 SDK 详情底部都有「导入本地版本」按钮（对本地 switch-only SDK，这是最直接的版本来源入口）。选择源目录、确认版本名（默认取目录名）后导入，语义是**移动**：

- 源目录（你选的那个版本目录）被**整体移动**到 `store/<sdk>/<version>/`，跨盘时自动复制后删除源目录——导入完成后原位置不再保留
- 不改变目录内部布局：你选的目录就是版本根，`bin_dir` 不可见时只发软警告（不影响导入，切换后 PATH 条目不生效可回头改 `bin_dir`）
- 版本名须唯一且不含路径分隔符（防路径穿越）；目标已存在时直接拒绝
- 导入后的版本与远程安装的版本完全一致：可切换、可卸载、可统计大小

CLI 目前没有对应的 import 子命令，等价做法就是上面的手动放置。

## 在 GUI 中管理自定义 SDK

CLI 能做的，GUI 设置页同样能做（两者读写同一份 `config.toml`）：

- **添加**：设置页 SDK 列表下方的添加表单——名称（必填）、版本发现 URL、下载 URL、二进制目录，即 `add-sdk` 的图形版
- **编辑源**：SDK 列表行的「编辑源」对话框，草稿式编辑——四个 URL 字段（版本发现主/备、下载主/备）对所有 SDK 开放（内置 SDK 换镜像源就走这里）；`bin_dir`、环境变量、额外 PATH 三块**仅自定义 SDK 可编辑**。对话框内「确定」只更新草稿，点列表下方「保存源配置」才写入 `config.toml`
- **删除**：自定义 SDK 可从列表移除（等价 `remove-sdk`），内置 SDK 只显示不可删
- **SDK 页行为差异**：按前述统一口径判断——`download_url` 与 `version_url` 皆空的 SDK 不显示「安装新版本」按钮，只显示「导入本地版本」；本地版本列表为空时的引导文案也会指向导入

## 移除自定义 SDK

```bash
sdkm config remove-sdk <NAME>
```

内置 SDK 不可移除（共 42 个，完整名单见下一节）。

移除只删除 `config.toml` 中的条目，不会删除 `store/` 下已下载的文件。

## 内置 SDK 名单（按类别）

sdkm 内置 42 个 SDK，按用途分四类。想让某个工具从"自定义"升级为"所有人开箱可用"的内置 SDK，见 [SDK 接入说明](./sdk-onboarding.md)。

### 语言 SDK / Runtime

| SDK | 主命令 | 备注 |
|:---|:---|:---|
| `java` | `java`、`javac` | 四家供应商（Temurin/Zulu/Corretto/Liberica），版本号写法见 [commands.md](./commands.md#sdkm-install) |
| `node` | `node`、`npm` | Windows 二进制在根目录，Unix 在 `bin/` |
| `python` | `python`、`python3` | python-build-standalone 构建，Windows 额外 PATH 含 `Scripts` |
| `go` | `go`、`gofmt` | 备源 `golang.google.cn` |
| `deno` | `deno` | GitHub Releases |

### 开发工具链

| SDK | 主命令 | 备注 |
|:---|:---|:---|
| `maven` | `mvn` | 无版本发现源，只能精确版本安装（不支持模糊匹配 / `list -r`） |
| `cmake` | `cmake` | 二进制在 `bin/` 子目录 |
| `helm` | `helm` | 组合模式：GH 版本列表 + 官方 CDN 模板下载 |
| `terraform` | `terraform` | 组合模式（同上） |
| `uv` | `uv`、`uvx` | GitHub Releases |
| `bun` | `bun` | GitHub Releases |
| `pnpm` | `pnpm` | GitHub Releases |
| `golangci-lint` | `golangci-lint` | GitHub Releases |
| `just` | `just` | GitHub Releases |
| `task` | `task` | GitHub Releases |
| `watchexec` | `watchexec` | GitHub Releases |
| `helmfile` | `helmfile` | GitHub Releases |
| `packer` | `packer` | 组合模式（Go 风格 arch 命名） |

### 开发辅助 CLI

| SDK | 主命令 | 备注 |
|:---|:---|:---|
| `ripgrep` | `rg` | 资产名前缀是 `ripgrep`（与主命令不同） |
| `fzf` | `fzf` | GitHub Releases |
| `lazygit` | `lazygit` | GitHub Releases |
| `gh` | `gh` | GitHub CLI，二进制在 `bin/` 子目录 |
| `k9s` | `k9s` | GitHub Releases |
| `claude-code` | `claude` | 资产名前缀是 `claude` |
| `fd` | `fd` | GitHub Releases |
| `bat` | `bat` | GitHub Releases |
| `eza` | `eza` | GitHub Releases |
| `delta` | `delta` | GitHub Releases |
| `starship` | `starship` | GitHub Releases |
| `lazydocker` | `lazydocker` | GitHub Releases |
| `stern` | `stern` | Kubernetes 多 Pod 日志 |
| `dive` | `dive` | 容器镜像分析 |
| `grpcurl` | `grpcurl` | gRPC 调试 |
| `temporal` | `temporal-server` | Temporal 服务端二进制 |
| `age` | `age`、`age-keygen` | 加密工具 |

### 归档式中间件 / 服务器软件（选择性支持）

只托管"解压即用、自己启动"的归档版，不负责服务注册与实例编排（见 [接入标准](./sdk-onboarding.md#一个工具能不能接入)）。

| SDK | 主命令 | 备注 |
|:---|:---|:---|
| `tomcat` | `catalina` | `add_to_path = false`，切换只设 `CATALINA_HOME`；版本源是 Apache HTML 目录 |
| `caddy` | `caddy` | GitHub Releases |
| `traefik` | `traefik` | GitHub Releases |
| `etcd` | `etcd`、`etcdctl` | GitHub Releases |
| `consul` | `consul` | 组合模式 |
| `vault` | `vault` | 组合模式 |
| `duckdb` | `duckdb` | 资产名前缀是 `duckdb_cli` |

> 名单随版本演进，以 `sdkm config list` 实际输出为准。

## 内置 SDK 下载源参考

注册自定义 SDK 时可参考内置 SDK 的源配置（`sdkm config list` 可看全部现值）：

| SDK | 版本发现源 | 下载源 |
|:---|:---|:---|
| Java | 四家供应商 API：Adoptium（默认 Temurin）/ Azul Zulu / GitHub `corretto-*` tags / BellSoft API | 各供应商 API 返回的平台直链 |
| Node.js | nodejs.org/dist/index.json | `nodejs.org/dist/{version}/...`（`{os}`=`win/darwin/linux`） |
| Python | astral-sh uv download-metadata（备源 GitHub API） | python-build-standalone releases（`{release_tag}`/`{platform}`） |
| Maven | （无） | `dlcdn.apache.org/maven/...`（`{version}`/`{ext}`） |
| Tomcat | `archive.apache.org/dist/tomcat/`（HTML 目录索引，多请求） | dlcdn 模板（当前版本 CDN，备源 archive；404 不重试直接回退），可换镜像 |
| Go | go.dev/dl/?mode=json | `go.dev/dl/go{version}.{os}-{arch}.{ext}`（备源 golang.google.cn） |
| 其余 30+ 引擎型 | GitHub Releases API（标准 B） | 资产直链（helm/terraform 为组合模式：GH 版本列表 + 官方 CDN 模板下载） |

> **两点特例**
> 1. **Java 只有部分场景走下载模板**：`sdkm install java 21`（默认供应商 + 裸大版本）仍渲染 `sdk.java.download_url` 模板，可改成 Adoptium 镜像；一旦指定精确版本或供应商（`21.0.13-zulu`、`amzn@21`、`librca@21`），下载直链来自各家 API，模板不参与（GitHub 托管的直链仍受 `network.github_download_mirror` 影响）。`sdk.java.version_url` 用于取可用大版本列表。
> 2. **内置 SDK 的新增不影响自定义注册**：二进制升级后 config 会自动补全缺失的内置条目（`ensure_builtin_sdks`），自定义 SDK 条目不受影响。
