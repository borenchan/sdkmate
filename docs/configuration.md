# 配置详解

> 读者：普通用户。

本文档详述 `config.toml` 的结构、每个配置项的含义、类型校验规则与脱敏策略。命令用法见 [commands.md](./commands.md)。

sdkm 的配置文件位于可执行文件同目录下的 `config.toml`（home 目录发现 = 运行中可执行文件的父目录，使工具可随目录移植）。可用 `sdkm config edit` 直接编辑，保存时自动校验 TOML 语法。

## 配置文件结构

```toml
# 顶层：符号链接目录（切换后的 SDK 入口）
# 默认不写此行 = 跟随 sdkm home（<sdkm_home>/links）；显式设置则用自定义路径
# symlink_dir = "/home/user/.sdkm/links"
# 顶层：托管环境变量文件（仅 Unix；Windows 走注册表不需要）
# 默认不写此行 = 跟随 home（<sdkm_home>/sdkm_env.sh | sdkm_env.fish）
# env_file = "~/.devrc"

[network]
proxy = ""                  # HTTP/HTTPS/SOCKS5 代理 URL，空 = 不使用代理
ssl_verify = true           # 是否校验 SSL 证书（TLS 后端为 rustls，走系统 CA bundle）
connect_timeout = 30        # 连接超时（秒）
cache_ttl_secs = 3600       # 版本接口缓存 TTL（秒），0 = 每次都拉取
github_token = ""           # GitHub PAT，提升 API 限速（60/hr → 5000/hr）
github_download_mirror = "" # GitHub 下载镜像前缀，多个用逗号分隔（如 "https://ghfast.top/, https://gh-proxy.com/"），空 = 关闭

# 每个 [[sdk]] 是一个 SDK 条目，内置 42 个 + 用户自定义
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
| `symlink_dir` | Path | 是 | `<sdkm_home>/links`（跟随 home） | 符号链接目录，切换 SDK 后各 SDK 的激活入口都在此目录下。**省略/删除 = 跟随 sdkm home**（exe 移到哪 links 就在哪，跨平台一致、用户级可写、绿色便携）；显式设置则用自定义路径（如多用户共享 `/opt/sdkm/links`） |
| `env_file` | Path | 是 | `<sdkm_home>/sdkm_env.sh` / `sdkm_env.fish`（跟随 home） | **仅 Unix**：sdkm 托管 PATH/环境变量写入的环境文件。profile 里只保留一行 source 该文件。可自定义指向用户自有文件（如 `~/.devrc`），此时 sdkm 只维护 `# >>> sdkm env >>>` / `# <<< sdkm env <<<` 标记块内的内容，块外不动。**自定义文件只适用于单一 shell**（块内是写入时那个 shell 的语法，bash/fish 混用会互相解析失败）；多 shell 用户请保持默认的分 shell 文件。**修改后自动生效**：sdkm 会把旧文件的托管块内容迁移到新文件，并把 profile 里的 source 行重定向到新路径（旧文件保留不删，可自行删除）。设为空串 `""` = 恢复默认（等价 `sdkm config delete env_file`）；必须是文件路径，已存在的目录会被拒绝 |

### `[network]` 网络配置

| 键 | 类型 | 可删除 | 默认 | 说明 |
|:---|:---|:---:|:---|:---|
| `proxy` | Url | 是 | (空) | 代理 URL，支持 `http://`、`https://`、`socks5://` 三种协议 |
| `ssl_verify` | Bool | 否 | `true` | 是否校验 TLS 证书；自签名/内网镜像可设为 `false` |
| `connect_timeout` | U32 | 否 | `30` | 连接超时秒数，范围 `[1, 600]` |
| `cache_ttl_secs` | U32 | 否 | `3600` | 远程版本列表缓存有效期（秒），范围 `[0, 86400]`；`0` 表示总是拉取最新 |
| `github_token` | Token | 是 | (空) | GitHub 个人访问令牌。Python 备源等走 GitHub API 的请求会被限速到 60/hr，配置 token 后提升至 5000/hr。输出时自动脱敏 |
| `github_download_mirror` | Url 列表 | 是 | (空) | GitHub 下载镜像前缀，逗号/空白分隔多个（如 `https://ghfast.top/, https://gh-proxy.com/`）。白名单域名（`github.com`、`raw/objects/codeload/gist.githubusercontent.com`）的版本数据与 SDK 包下载会展开为候选源链：「配置的镜像前缀 → Temurin 清华 Adoptium 镜像（仅最新 patch）→ 高校 github-release 镜像（清华/北外/中科大，白名单仓库 + 仅最新 release）→ 直连兜底」，下载前并行探测选最快、失败自动切换并跨源续传；有备援时单源只试 1 次（限速/4xx/无响应立即切），仅最后一个兜底源允许 1 次重试；配置了 `proxy` 时直连优先（走代理，不再绕镜像）。**`api.github.com` 不在白名单内**。首次 `init` 检测到中国大陆时区会自动写入多个默认镜像，设空关闭 |

> **缓存策略**：远程版本列表采用「缓存优先 + TTL」——未过期直接用本地缓存，过期才请求 API；API 失败时退化为返回过期缓存，保证离线可用。

### `[[sdk]]` SDK 条目

每个 `[[sdk]]` 描述一个 SDK 的版本发现与下载来源。内置 42 个（java/node/python/go/maven/tomcat 等运行时与工具链 + bun/pnpm/deno/uv/claude-code/cmake/gh/helm/terraform 等 30+ 工具，完整名单见 [custom-sdk.md · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)），用户可通过 `sdkm config add-sdk` 添加自定义条目（见 [custom-sdk.md](./custom-sdk.md)）。

| 键 | 类型 | 可删除 | 说明 |
|:---|:---|:---:|:---|
| `name` | — | — | SDK 唯一名称，用于命令行引用 |
| `version_url` | Url | 内置否/自定义是 | 版本发现主源 URL，返回可用版本列表 |
| `version_fallback_url` | Url | 同上 | 版本发现备源，主源失败时回退 |
| `download_url` | UrlTemplate | 内置否/自定义是 | 下载主源 URL 模板，支持 `{version}` 等占位符。**与 `version_url` 至少一个非空**才可远程安装；两者皆空 = 本地 switch-only SDK（仅切版本，不远程安装） |
| `download_fallback_url` | UrlTemplate | 内置否/自定义是 | 下载备源 URL 模板，主源失败时回退 |
| `current_version` | NonEmptyString | 同上 | 当前激活版本（由 `switch` 自动维护，一般不手动改） |
| `bin_dir` | FreeString | **否（任何 SDK）** | 二进制所在子目录名；**空值/省略 = 二进制在 SDK 根目录**（如 Windows 下的 Node.js / Python）；内置 SDK 已按平台预设（Linux/macOS 的 Node 与 Python 为 `bin`，Java/Maven 全平台为 `bin`）；必填 |
| `extra_vars` | NonEmptyString | 同上 | 额外环境变量键值表，值支持模板渲染（如 `JAVA_HOME = "{sdk_dir}"`） |
| `extra_paths` | Path | 同上 | 额外 PATH 条目（相对符号链接目录，可多条） |
| `os_style` / `arch_style` | OsStyle / ArchStyle | 同上 | 下载模板 `{os}` / `{arch}` 的命名风格（仅模板型 SDK 用到） |
| `add_to_path` | Bool | 同上 | `true`（默认）= 切换时把 `<links>/<sdk>/<bin_dir>` 写入 PATH；`false` = 只设 `extra_vars` 不碰 PATH（内置 Tomcat 即如此，避免 `startup`/`version` 这类通用脚本名污染全局 PATH） |

> **内置 SDK 保护**：42 个内置 SDK（完整名单见 [custom-sdk.md · 内置 SDK 名单](./custom-sdk.md#内置-sdk-名单按类别)）的所有字段都不可 `delete`，也不可 `remove-sdk`，只能用 `set` 修改。`bin_dir` 对任意 SDK 都是必填字段，不可删除；`download_url` 内置 SDK 必填不可删，**自定义 SDK 可省略/可删**。**能否远程安装的唯一口径**是 `can_install_remotely()`：`download_url` 或 `version_url` 至少一个非空即可远程安装（GitHub Releases 形态只填 `version_url` 也能装）；两者皆空才是本地 switch-only（GUI 隐藏「安装新版本」、只留「导入本地版本」）。

## 键名格式（点分隔）

`sdkm config set/get/delete` 用点分隔字符串定位配置项：

| 键名格式 | 示例 |
|:---|:---|
| `symlink_dir` | `symlink_dir` |
| `env_file` | `env_file` |
| `network.<field>` | `network.proxy`、`network.cache_ttl_secs` |
| `sdk.<name>.<field>` | `sdk.java.download_url`、`sdk.node.bin_dir` |
| `sdk.<name>.extra_vars.<KEY>` | `sdk.java.extra_vars.JAVA_HOME` |
| `sdk.<name>.extra_paths.<N>` | `sdk.java.extra_paths.0`（按索引，从 0 开始） |

SDK 字段 `<field>` 取值：`version_url` / `version_fallback_url` / `download_url` / `download_fallback_url` / `current_version` / `bin_dir` / `os_style` / `arch_style` / `add_to_path`。

无效键名会报错并列出全部合法键名清单。

## 类型校验规则

每个配置项绑定一个 `ValueType`，`set` 时按类型校验后才写入。新增字段只需声明类型即自动获得校验。

| 类型 | 规则 | 适用字段示例 |
|:---|:---|:---|
| `Url` | 合法 URL，协议限 `http` / `https` / `socks5` | `network.proxy`、`sdk.*.version_url` |
| `UrlTemplate` | URL 模板，`{xxx}` 占位符替换为占位串后能通过 URL 校验 | `sdk.*.download_url` |
| `Bool` | 接受 `true/false/1/0/yes/no/on/off`（大小写不敏感，与 git 一致） | `network.ssl_verify` |
| `U32` | 正整数，范围 `[min, max]` | `network.connect_timeout` `[1,600]`、`cache_ttl_secs` `[0,86400]` |
| `Path` | 非空字符串（不要求路径存在） | `symlink_dir`、`env_file`、`extra_paths` |
| `Token` | 非空字符串，**输出脱敏**（仅显示前 4 字符 + `***`） | `network.github_token` |
| `NonEmptyString` | 非空字符串 | `current_version`、`extra_vars` 值 |
| `FreeString` | 允许空值，**禁止路径分隔符** `/` `\`；空值表示「二进制在根目录」 | `bin_dir` |

## 写入安全

- **原子写入**：所有配置写操作采用「写入临时文件 → 重命名」模式，避免写入中途崩溃导致配置文件损坏。
- **快照回滚**：`set` / `delete` / `add-sdk` / `remove-sdk` 失败时，自动恢复到操作前的配置（内存级 + 磁盘原始内容级双重恢复）。
- **TOML 校验**：`edit` 保存后自动重新解析 `config.toml`，语法错误会提示但不破坏现有文件。

## GUI 偏好文件（gui.toml）

桌面应用 `sdkm-gui` 的界面偏好**不存在 `config.toml` 里**，而是单独写 `<sdkm_home>/gui.toml`（与 CLI 完全分离，CLI 不读不写）：

| 键 | 取值 | 说明 |
|:---|:---|:---|
| `lang` | `Zh`（默认）/ `En` | 界面语言 |
| `theme_mode` | `System`（默认）/ `Light` / `Dark` | 主题模式 |
| `default_page` | `overview`（默认）/ `sdk` / `projects` / `settings` / `diagnostics` | 启动页 |
| `density` | `Standard`（默认）/ `Compact` | 界面密度 |

这些项在 GUI 的「设置 → 界面偏好设置」里修改，一般不需要手动编辑。业务配置（网络、SDK 源、目录）仍全部走 `config.toml`，CLI 与 GUI 共享。
