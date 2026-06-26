# 配置详解

本文档详述 `config.toml` 的结构、每个配置项的含义、类型校验规则与脱敏策略。命令用法见 [commands.md](./commands.md)。

sdkm 的配置文件位于可执行文件同目录下的 `config.toml`（home 目录发现 = 运行中可执行文件的父目录，使工具可随目录移植）。可用 `sdkm config edit` 直接编辑，保存时自动校验 TOML 语法。

## 配置文件结构

```toml
# 顶层：符号链接目录（切换后的 SDK 入口）
symlink_dir = "C:\\Program Files\\sdkm"   # Windows 默认
# symlink_dir = "/usr/local/sdkm"          # Unix 默认

[network]
proxy = ""                  # HTTP/HTTPS/SOCKS5 代理 URL，空 = 不使用代理
ssl_verify = true           # 是否校验 SSL 证书
connect_timeout = 30        # 连接超时（秒）
cache_ttl_secs = 3600       # 版本接口缓存 TTL（秒），0 = 每次都拉取
github_token = ""           # GitHub PAT，提升 API 限速（60/hr → 5000/hr）

# 每个 [[sdk]] 是一个 SDK 条目，内置 4 个 + 用户自定义
[[sdk]]
name = "java"
version_url = "https://api.adoptium.net/v3/info/available_releases"
download_url = "https://api.adoptium.net/v3/binary/latest/{feature_version}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse"
bin_dir = "bin"
current_version = "21"
extra_vars = { JAVA_HOME = "{sdk_dir}" }
extra_paths = []
```

## 配置项含义

### 顶层

| 键 | 类型 | 可删除 | 默认 | 说明 |
|:---|:---|:---:|:---|:---|
| `symlink_dir` | Path | 否 | Win: `C:\Program Files\sdkm` / Unix: `/usr/local/sdkm` | 符号链接目录，切换 SDK 后各 SDK 的激活入口都在此目录下 |

### `[network]` 网络配置

| 键 | 类型 | 可删除 | 默认 | 说明 |
|:---|:---|:---:|:---|:---|
| `proxy` | Url | 是 | (空) | 代理 URL，支持 `http://`、`https://`、`socks5://` 三种协议 |
| `ssl_verify` | Bool | 否 | `true` | 是否校验 TLS 证书；自签名/内网镜像可设为 `false` |
| `connect_timeout` | U32 | 否 | `30` | 连接超时秒数，范围 `[1, 600]` |
| `cache_ttl_secs` | U32 | 否 | `3600` | 远程版本列表缓存有效期（秒），范围 `[0, 86400]`；`0` 表示总是拉取最新 |
| `github_token` | Token | 是 | (空) | GitHub 个人访问令牌。Python 备源等走 GitHub API 的请求会被限速到 60/hr，配置 token 后提升至 5000/hr。输出时自动脱敏 |

> **缓存策略**：远程版本列表采用「缓存优先 + TTL」——未过期直接用本地缓存，过期才请求 API；API 失败时退化为返回过期缓存，保证离线可用。

### `[[sdk]]` SDK 条目

每个 `[[sdk]]` 描述一个 SDK 的版本发现与下载来源。内置 4 个（java/node/python/maven），用户可通过 `sdkm config add-sdk` 添加自定义条目（见 [custom-sdk.md](./custom-sdk.md)）。

| 键 | 类型 | 可删除 | 说明 |
|:---|:---|:---:|:---|
| `name` | — | — | SDK 唯一名称，用于命令行引用 |
| `version_url` | Url | 内置否/自定义是 | 版本发现主源 URL，返回可用版本列表 |
| `version_fallback_url` | Url | 同上 | 版本发现备源，主源失败时回退 |
| `download_url` | UrlTemplate | **否（任何 SDK）** | 下载主源 URL 模板，支持 `{version}` 等占位符，必填 |
| `download_fallback_url` | UrlTemplate | 内置否/自定义是 | 下载备源 URL 模板，主源失败时回退 |
| `current_version` | NonEmptyString | 同上 | 当前激活版本（由 `switch` 自动维护，一般不手动改） |
| `bin_dir` | FreeString | **否（任何 SDK）** | 二进制所在子目录名；**空字符串 = 二进制在 SDK 根目录**（如 Node.js、Windows 下的 Python）；必填 |
| `extra_vars` | NonEmptyString | 同上 | 额外环境变量键值表，值支持模板渲染（如 `JAVA_HOME = "{sdk_dir}"`） |
| `extra_paths` | Path | 同上 | 额外 PATH 条目（相对符号链接目录，可多条） |

> **内置 SDK 保护**：内置 SDK（java/node/python/maven）的所有字段都不可 `delete`，也不可 `remove-sdk`，只能用 `set` 修改。`download_url` 与 `bin_dir` 对任意 SDK 都是必填字段，不可删除。

## 键名格式（点分隔）

`sdkm config set/get/delete` 用点分隔字符串定位配置项：

| 键名格式 | 示例 |
|:---|:---|
| `symlink_dir` | `symlink_dir` |
| `network.<field>` | `network.proxy`、`network.cache_ttl_secs` |
| `sdk.<name>.<field>` | `sdk.java.download_url`、`sdk.node.bin_dir` |
| `sdk.<name>.extra_vars.<KEY>` | `sdk.java.extra_vars.JAVA_HOME` |
| `sdk.<name>.extra_paths.<N>` | `sdk.java.extra_paths.0`（按索引，从 0 开始） |

SDK 字段 `<field>` 取值：`version_url` / `version_fallback_url` / `download_url` / `download_fallback_url` / `current_version` / `bin_dir`。

无效键名会报错并列出全部合法键名清单。

## 类型校验规则

每个配置项绑定一个 `ValueType`，`set` 时按类型校验后才写入。新增字段只需声明类型即自动获得校验。

| 类型 | 规则 | 适用字段示例 |
|:---|:---|:---|
| `Url` | 合法 URL，协议限 `http` / `https` / `socks5` | `network.proxy`、`sdk.*.version_url` |
| `UrlTemplate` | URL 模板，`{xxx}` 占位符替换为占位串后能通过 URL 校验 | `sdk.*.download_url` |
| `Bool` | 接受 `true/false/1/0/yes/no/on/off`（大小写不敏感，与 git 一致） | `network.ssl_verify` |
| `U32` | 正整数，范围 `[min, max]` | `network.connect_timeout` `[1,600]`、`cache_ttl_secs` `[0,86400]` |
| `Path` | 非空字符串（不要求路径存在） | `symlink_dir`、`extra_paths` |
| `Token` | 非空字符串，**输出脱敏**（仅显示前 4 字符 + `***`） | `network.github_token` |
| `NonEmptyString` | 非空字符串 | `current_version`、`extra_vars` 值 |
| `FreeString` | 允许空值，**禁止路径分隔符** `/` `\`；空值表示「二进制在根目录」 | `bin_dir` |

## 写入安全

- **原子写入**：所有配置写操作采用「写入临时文件 → 重命名」模式，避免写入中途崩溃导致配置文件损坏。
- **快照回滚**：`set` / `delete` / `add-sdk` / `remove-sdk` 失败时，自动恢复到操作前的配置（内存级 + 磁盘原始内容级双重恢复）。
- **TOML 校验**：`edit` 保存后自动重新解析 `config.toml`，语法错误会提示但不破坏现有文件。
