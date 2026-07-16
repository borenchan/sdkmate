# 自定义 SDK

sdkm 内置支持 Java、Node.js、Python、Maven 四个 SDK。除此之外，任何「能从 URL 下载、解压后得到带可执行文件的目录」的工具，都可以注册为自定义 SDK，纳入 `sdkm` 统一管理——安装、切换、环境变量一视同仁。

本文档讲解如何用 `sdkm config add-sdk` 注册自定义 SDK，以及下载 URL 模板中的占位符系统。配置项完整含义见 [configuration.md](./configuration.md)。

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

内置 SDK（java/node/python/maven）不可移除。移除只删除 `config.toml` 中的条目，不会删除 `store/` 下已下载的文件。

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
