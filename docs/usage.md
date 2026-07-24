# 详细用法

本节是 sdkm 的完整使用文档入口。如果你只想要「30 秒跑起来」，看 [项目 README](../README.md#快速开始) 即可。

## 文档导航

| 文档 | 内容 |
|:---|:---|
| [commands.md](./commands.md) | 每个子命令的参数、别名、行为与示例（init / install / list / switch / current / config / self） |
| [configuration.md](./configuration.md) | `config.toml` 结构、每个配置项含义、类型校验规则、写入安全机制 |
| [custom-sdk.md](./custom-sdk.md) | 用 `add-sdk` 注册任意工具为自定义 SDK、URL 模板占位符系统 |

## 30 秒快速上手

```bash
# 1. 初始化（首次使用）
sdkm init

# 2. 安装一个 SDK（支持模糊匹配，自动切换）
sdkm install java 21

# 3. 或者把已有的本地 SDK 交给 sdkm 托管：
#    放到 <sdkm所在目录>/store/java/21/ 下，sdkm 会自动发现

# 4. 列出与切换
sdkm list                  # 查看所有已安装 SDK + 当前版本
sdkm list node -r          # 交互式浏览远程 Node.js 版本，按 i 安装
sdkm switch java 17        # 切换到本地已安装的 Java 17
sdkm current               # 查看当前激活版本
```

## 把已有 SDK 交给 sdkm 托管

sdkm 不强制从远程安装。你可以把已经装好的 SDK 直接放进 `store/` 目录，sdkm 会自动发现并纳入管理：

```
<sdkm所在目录>/store/java/21/      # JDK 21
<sdkm所在目录>/store/node/22/      # Node.js 22
<sdkm所在目录>/store/python/3.12/  # Python 3.12
```

放进目录后 `sdkm list java` 即可看到，`sdkm switch java 21` 即可切换。**无需修改系统环境变量、无需重启终端**——切换后通过符号链接 + PATH 注入实时生效。

## 交互式 TUI

`sdkm list <sdk>` 和 `sdkm list <sdk> -r` 会进入交互式版本选择器：

- `↑` / `↓` 或 `k` / `j`：导航
- 本地选择器：`Enter` / `s` 切换版本
- 远程选择器：`i` 安装、`s` 切换
- `q` / `Esc` / `Ctrl+C`：退出

状态标记：`✅` 当前激活 / `📦` 已安装 / 空白 = 未安装。

## 配置

`sdkm config` 系列：

```bash
sdkm config list                              # 列出全部配置
sdkm config set network.proxy http://127.0.0.1:7890   # 设置代理
sdkm config set network.cache_ttl_secs 0      # 关闭版本缓存（每次都拉最新）
sdkm config edit                              # 用编辑器直接改 config.toml
```

完整配置项含义与校验规则见 [configuration.md](./configuration.md)。

## 自定义 SDK

内置只覆盖 Java / Node.js / Python / Maven / Go。任何能从 URL 下载解压的工具都能注册：

```bash
sdkm config add-sdk mytool \
  --download-url "https://example.com/mytool/{version}/mytool-{version}-{os}-{arch}.{ext}" \
  --bin-dir bin
```

详见 [custom-sdk.md](./custom-sdk.md)。

---

## ⚠️ 已知限制

坦诚面对当前状态，以下是使用时的注意事项：

- **Maven 无远程版本发现**：Maven 只有下载模板、没有版本发现接口，因此 `sdkm install maven <version>` 必须给精确版本号（如 `3.9.9`），不支持模糊匹配，`sdkm list maven -r` 会报错。自定义 SDK 不填 `--version-url` 时同理。
- **Windows 需管理员权限**：Windows 下环境变量与系统 PATH 写入 `HKEY_LOCAL_MACHINE`，运行 `init` / `switch` 需管理员权限。
- **Python 远程列表**：主源（uv metadata）完整；备源（GitHub API）受 `per_page=100` 限制，仅返回最近 100 个 release，主源正常时不触发。
- **Java macOS aarch64 无 jdk8 包**：Adoptium 不提供 jdk8 的 macOS aarch64 构建（jdk8 在 macOS 仅 x64），Apple Silicon 上 `sdkm install java 8` 会报错，改用 `17`/`21` 等支持 aarch64 的版本。
