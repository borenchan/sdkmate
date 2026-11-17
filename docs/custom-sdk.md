# 自定义 SDK

sdkm 内置支持 Java、Node.js、Python、Maven、Go，以及 Android、Dart、.NET、Flutter、Gradle、LLVM、MinGW、Rust 等开发工具。内置身份由共享注册表决定，CLI 与桌面端使用同一份定义；内置管理能力与远程下载能力分别判断。

其他工具可以注册为自定义 SDK，纳入统一管理。若有可直接解压的下载归档，还可以配置远程安装。

本文档讲解如何用 `sdkm config add-sdk` 注册自定义 SDK，以及下载 URL 模板中的占位符系统。配置项完整含义见 [configuration.md](./configuration.md)。

## 内置开发 SDK

以下 SDK 无需 `config add-sdk`，新配置自动包含；升级后缺少的条目会自动补全。已有同名条目会直接获得内置身份，但其版本、布局、环境变量和源设置保留，包括用户主动清空的源；不会用默认下载源覆盖旧配置。

| SDK 名称 | 默认命令目录 | 默认环境变量 / 额外 PATH | 默认远程能力 |
|:---|:---|:---|:---|
| `android` | `platform-tools` | `ANDROID_HOME`、`ANDROID_SDK_ROOT`；`cmdline-tools/latest/bin`、`emulator` | 管理本地完整 SDK |
| `dart` | `bin` | — | 官方 stable 归档，精确版本安装 |
| `dotnet` | 根目录 | `DOTNET_ROOT` | 官方 SDK 归档，精确版本安装 |
| `flutter` | `bin` | `FLUTTER_ROOT` | 管理本地完整 SDK |
| `gradle` | `bin` | `GRADLE_HOME` | 官方 bin.zip，精确版本安装 |
| `llvm` | `bin` | — | 管理本地完整工具链 |
| `mingw` | `bin` | — | 管理本地完整工具链（Windows） |
| `rust` | `bin` | — | 管理本地已安装工具链 |

Dart、.NET、Gradle 尚无默认远程版本列表，需提供完整版本号：

```bash
sdkm install dart 3.9.4
sdkm install dotnet 8.0.417
sdkm install gradle 8.8
```

Dart 使用 Windows/Linux/macOS 的 zip；.NET 使用 Windows zip 或 Unix tar.gz，并区分 Linux GNU/musl；Gradle 各平台均使用 zip。默认下载模板面向 x64/arm64；其他架构可修改源配置。已有同名本地条目若要启用下载，可按下方源参考用 `sdkm config set sdk.<name>.download_url <模板>` 配置。

本地内置 SDK 按 `store/<SDK>/<版本>/` 放置后即可 `list` / `switch` / `use`。它们拥有与其他内置 SDK 相同的分类和配置删除保护，仍可修改字段或补充下载源。目录必须保留完整布局：

- Android 放完整 SDK 根，包含 `platform-tools/`、`cmdline-tools/latest/`、平台和构建工具等组件；单独的 command-line tools 压缩包不构成完整 SDK。
- Flutter 保留 `bin/`、缓存和 Git 元数据；目标平台构建工具另行准备。
- Rust 放真实工具链根（如 rustup 的 `toolchains/<工具链>/`），其中有 `bin/rustc` 和 `bin/cargo`；不要放 `.cargo/bin` 的代理程序或尚未执行组件安装的发行归档。
- .NET 本地导入需保留 `dotnet`、`host/`、`sdk/`、`shared/` 等根目录组件；只复制 `sdk/<版本>/` 不构成可运行的 SDK。
- Gradle 需要 JDK；Rust/MSVC、LLVM、MinGW、Flutter 的系统依赖或链接器不随版本切换自动安装。

## 注册自定义 SDK

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

| 参数 | 必填 | 说明 |
|:---|:---:|:---|
| `<NAME>` | 是 | SDK 唯一名称，不可与已有 SDK 重名 |
| `--download-url` | 否 | 下载主源 URL 模板，支持 `{version}` 等占位符。**省略 = 本地 switch-only SDK**（见下节） |
| `--bin-dir` | 否 | 二进制所在子目录名；**省略 = 二进制在 SDK 根目录**（如 Node.js）。传值则必须是简单目录名（`bin`、`Scripts`），不能含 `/` 或 `\` |
| `--version-url` | 否 | 版本发现主源 URL；不填则该 SDK 只支持精确版本安装，不支持模糊匹配与 `list -r` |
| `--version-fallback-url` | 否 | 版本发现备源 URL |
| `--download-fallback-url` | 否 | 下载备源 URL 模板 |
| `--extra-var` | 否 | 额外环境变量，`KEY=VALUE` 格式，可重复 |
| `--extra-path` | 否 | 额外 PATH 条目（相对符号链接目录），可重复 |

注册后即可像内置 SDK 一样使用：

```bash
sdkm install mytool 1.2.3
sdkm switch mytool 1.2.3
sdkm current mytool
sdkm list mytool          # 本地版本选择器
```

### 示例：注册一个简单工具

假设有个名为 `mytool` 的工具，下载地址形如 `https://example.com/mytool/1.2.3/mytool-1.2.3-linux-x64.tar.gz`，解压后二进制在 `bin/` 子目录：

```bash
sdkm config add-sdk mytool \
  --download-url "https://example.com/mytool/{version}/mytool-{version}-{os}-{arch}.{ext}" \
  --bin-dir bin
```

### 示例：带版本发现与环境变量

```bash
sdkm config add-sdk groovy \
  --version-url "https://example.com/groovy/versions.json" \
  --download-url "https://example.com/groovy/{version}/apache-groovy-{version}-bin.{ext}" \
  --bin-dir bin \
  --extra-var GROOVY_HOME="{sdk_dir}"
```

### 本地 switch-only SDK（不远程安装）

只想让 sdkm 托管一个**已经在本地的**工具目录、用 `sdkm switch` 切版本、不通过 sdkm 远程下载安装时，**省略 `--download-url`** 即可：

```bash
sdkm config add-sdk mylocal --bin-dir bin
```

注册后手动把各版本目录放到 `store/mylocal/<version>/`（如 `store/mylocal/1.0.0/`、`store/mylocal/2.0.0/`），即可用 `sdkm switch mylocal 1.0.0` / `sdkm current mylocal` / `sdkm list mylocal` 切换与查看。

> 这种 SDK **不能** `sdkm install`（无下载源），尝试会明确报错并提示放置目录或补 `download_url`。若日后需要远程安装，`sdkm config set sdk.mylocal.download_url <url>` 补上即可。

## 移除自定义 SDK

```bash
sdkm config remove-sdk <NAME>
```

内置 SDK（包括 java/node/python/go/maven、上述八种开发 SDK 及其他注册表条目）不可移除。移除只删除 `config.toml` 中的条目，不会删除 `store/` 下已下载的文件。

## URL 模板占位符

下载 URL（`download_url` / `download_fallback_url`）和环境变量值都支持占位符，在安装/切换时由 `TemplateRenderer` 自动替换。

### 通用占位符

| 占位符 | 含义 | 示例值 |
|:---|:---|:---|
| `{version}` | 完整版本号 | `21`、`v20.11.0`、`3.12.0` |
| `{os}` | 操作系统名（默认风格） | `windows` / `linux` / `darwin` |
| `{arch}` | CPU 架构（默认风格） | `x64` / `arm64` / `x86` |
| `{ext}` | 平台压缩包扩展名 | Windows: `zip`；Linux/macOS: `tar.gz` |

### SDK 专用占位符

不同 SDK 的下载源用不同的命名风格，sdkm 按需提供：

| 占位符 | 含义 | 用于 |
|:---|:---|:---|
| `{feature_version}` | 大版本号（如 `21`） | Java（Adoptium） |
| `{release_tag}` | 构建日期标签（如 `20241216`，动态发现） | Python（python-build-standalone） |
| `{platform}` | 平台三元组（如 `x86_64-pc-windows-msvc`，自动检测） | Python |

### 路径类占位符（用于环境变量值）

| 占位符 | 含义 |
|:---|:---|:---|
| `{sdk_dir}` | 当前 SDK 的符号链接目录绝对路径（即激活版本目录） |
| `{sdkm_home}` | sdkm 可执行文件所在目录（home 目录） |
| `{sdks_install_dir}` | SDK 安装根目录（`<home>/store/`） |

> 经典用法：`--extra-var JAVA_HOME="{sdk_dir}"` —— 切换版本时 `JAVA_HOME` 自动指向新的激活目录。

## 内置 SDK 下载源参考

注册自定义 SDK 时可参考内置 SDK 的源配置：

| SDK | 版本发现源 | 下载源 |
|:---|:---|:---|
| Java | Adoptium available_releases API | Adoptium binary latest API（`{feature_version}`/`{os}`/`{arch}`） |
| Node.js | nodejs.org/dist/index.json | `nodejs.org/dist/{version}/...`（`{os}`=`win/darwin/linux`） |
| Python | astral-sh uv download-metadata（备源 GitHub API） | python-build-standalone releases（`{release_tag}`/`{platform}`） |
| Maven | （无） | `dlcdn.apache.org/maven/...`（`{version}`/`{ext}`） |
| Go | go.dev/dl/?mode=json | `go.dev/dl/go{version}.{os}-{arch}.{ext}`（备源 golang.google.cn） |
| Dart | （无，仅精确版本） | `storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-<windows/linux/macos>-{arch}-release.zip` |
| .NET SDK | （无，仅精确版本） | `builds.dotnet.microsoft.com/dotnet/Sdk/{version}/dotnet-sdk-{version}-<win/linux/linux-musl/osx>-{arch}.<zip/tar.gz>` |
| Gradle | （无，仅精确版本） | `services.gradle.org/distributions/gradle-{version}-bin.zip` |
| Android / Flutter / LLVM / MinGW / Rust | （无） | 默认管理本地完整 SDK，无远程源 |

## GitHub Releases 接入标准（标准 B）

任何通过 GitHub Releases 分发压缩包的工具，只需一条 `version_url` 即可接入，**无需配置下载模板**：

```bash
sdkm config add-sdk <name> --version-url "https://api.github.com/repos/<owner>/<repo>/releases"
```

引擎自动完成（零配置）：

- **版本解析**：从 `tag_name` 剥离前缀取版本号（`bun-v1.4.2` / `v2.9.6` / `0.12.11` 等形态通吃），
  自动过滤 `prerelease` / `draft`，自动补 `per_page=100`
- **平台资产选择**：从 `assets` 中按"OS 词 + arch 词 + 压缩包后缀"自动识别当前平台的 zip/tar.gz 直链
  （windows/win32、darwin/macos、x64/x86_64/amd64、arm64/aarch64 等命名惯例内置同义词表；
  `baseline`/`profile` 修饰变体与 musl/gnu 按运行环境自动取舍）
- **直链下载**：选中的 `browser_download_url` 直接作为下载地址，无需 `download_url` 模板

### 镜像与备源

GH 直链下载失败时自动回落 `download_fallback_url` 模板（国内网络建议配置）：

```bash
sdkm config set sdk.<name>.download_fallback_url \
  "https://ghfast.top/https://github.com/<owner>/<repo>/releases/download/v{version}/<name>-{os}-{arch}.zip"
```

`{version}` 为剥离前缀后的版本号；`{os}`/`{arch}` 风格可用 `sdk.<name>.os_style`（default/short/adoptium）
与 `sdk.<name>.arch_style`（default/adoptium/python/go）调整。

### 适用条件

- 资产为 zip 或 tar.gz（裸 exe 单文件暂不支持）
- 解压后二进制位于压缩包根目录或 `bin/` 子目录（`--bin-dir` 按实际布局指定）
- 主命令名与资产前缀一致（如 `bun` 前缀资产内的 `bun.exe`）；主命令名不同时用内置种子登记
  （如 claude-code 的资产前缀是 `claude`）
