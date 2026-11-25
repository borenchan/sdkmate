# 常见问题

> 读者：普通用户。

先跑一遍诊断，多数问题一步定位：

```bash
sdkm doctor        # CLI：打印完整诊断报告
# GUI：诊断与维护 → 环境诊断 → 重新检查
```

报告分节说明见 [commands.md · sdkm doctor](./commands.md#sdkm-doctor)。提 issue 时整份粘贴即可。

## 切换了版本，但终端里还是旧版本

**原因**：`switch` 改的是全局符号链接与 `config.toml`；当前 shell 的 PATH 由 hook 在**每次提示符渲染**时重建。

**处理**：

1. 按一下回车（触发 hook → `sdkm env` → 新 PATH 生效）。
2. 仍不行：确认 hook 已注入（`sdkm doctor` 的 `shell & env persistence` 节显示 `hook registered`）。未注入就重跑 `sdkm init`。
3. 新开的终端也没生效：`doctor` 的 `health checks` 里看 `PATH: sdkm links entries`；缺失说明 PATH 持久化没写进去，重跑 `sdkm init` 或用 [手动注册 hook](./usage.md#手动注册-hook)。
4. 以上都正常，仍命中旧版本：见下一条 PATH 冲突。

## 明明切了版本，命令却指向另一个安装（PATH 冲突）

**原因**：PATH 里还有系统包管理器 / 手工安装的同名命令，或旧版 sdkm 直写进 profile 的残留条目。

**处理**：

- 切版本时 sdkm 会警告 `Found existing SDK[...] path at '...' in PATH`——这些条目 sdkm 不删除（sdkm 注入的路径优先级最高，正常应被覆盖）。若发现没覆盖，检查该条目是否排在 sdkm 路径之前（`echo $PATH`）。
- 卸载 sdkm 时才会精准清理"路径在 sdkm home / `links/` 树内"的旧条目；其余请手动从 profile 删除。

## `sdkm install` 报 `bin dir mismatch for ...`

**原因**：包解压后的目录布局与 `bin_dir` 配置不一致，主命令不在预期位置。安装/导入本身已成功，但 PATH 指不到可执行文件。

**处理**：按警告里给出的位置修正：

```bash
sdkm config set sdk.<name>.bin_dir <实际子目录>
```

然后重新 `sdkm switch` 一次。

## Maven 不能用模糊版本 / `list maven -r` 报错

**原因**：Maven 没有版本发现接口，只能精确版本安装。

**处理**：给完整版本号，如 `sdkm install maven 3.9.9`。自定义 SDK 没配 `version_url` 时同理。

## Tomcat 下载卡住或极慢

**原因**：默认下载源 `dlcdn.apache.org` 在国内网络经常零数据卡死（下载器有首响应 8s / 静默 15s / 低速 60s 判卡死 + 候选源自动切换，但仍慢）。

**处理**：内置备源已自动回退华为云 Apache 镜像（全历史），通常无需手工配置。若仍想直接用华为云作主源：

```bash
sdkm config set sdk.tomcat.download_url "https://mirrors.huaweicloud.com/apache/tomcat/tomcat-{feature_version}/v{version}/bin/apache-tomcat-{version}.{ext}"
```

## 下载 GitHub 资源很慢 / GitHub API 限流

- **慢**：启用下载镜像。首次 `init` 检测到中国大陆时区会自动写入多个默认镜像（`ghfast.top` / `edgeone.gh-proxy.com` / `gh-proxy.com`）；也可手动配置任意多个：

  ```bash
  sdkm config set network.github_download_mirror "https://ghfast.top/, https://gh-proxy.com/"
  ```

  下载前会并行探测各候选源并选最快的，失败自动切换并断点续传；除配置的镜像前缀外，Temurin（Java）还会走清华 Adoptium 镜像（仅每个 feature 的最新 patch），其他 GitHub Release 资产会尝试清华/北外/中科大的 `github-release` 镜像（白名单仓库、仅最新 release，如 USTC 的 astral-sh Python），全部不可用时回退直连。只重写 `github.com` 等白名单域名，`api.github.com` 不受影响。
- **Node.js / Maven / Tomcat 官方源慢**：内置 SDK 已带华为云备源（Node `/nodejs/`、Maven/Tomcat `/apache/`），与官方源同名同路径，下载时探测选最快，无需手工配置。
- **限流**（`GitHub API returned HTTP 403/429`）：配置 token：

  ```bash
  sdkm config set network.github_token <your-pat>
  ```

  匿名 60 次/小时 → 带 token 5000 次/小时。

## 项目里 pin 了版本却不生效

**原因**：项目级 / 会话级都依赖 hook；另外 `use` 写入的版本必须本地已安装。

**处理**：

1. 先 `sdkm init` 注入 hook；未注入时只有 `switch`（全局）有效。
2. `sdkm current <sdk>` 看 `source` 列：`project (...)` 才对；显示 `global` 说明 pin 没被读到（检查是否在项目目录或其子目录、`.sdkm.toml` 键名是否正确）。
3. 手写 `.sdkm.toml` 用了未安装版本：`sdkm env` 会 stderr 警告并**回退全局**。先装该版本。

## 临时会话版本（`use --shell`）怎么取消

`SDKM_ACTIVE_*` 只有设置入口，没有 `use --shell --unset`。手动清除：

```bash
unset SDKM_ACTIVE_JAVA            # bash / zsh
set -e SDKM_ACTIVE_JAVA           # fish
Remove-Item Env:SDKM_ACTIVE_JAVA  # PowerShell
```

之后按回车，hook 重算环境即回到项目/全局层。

## Windows 报权限错误（os error 5 / 1314）

**原因**：sdkm 写 `HKEY_LOCAL_MACHINE` 的 PATH/环境变量、创建符号链接，需要**管理员权限**。

**处理**：以管理员身份运行终端或 `sdkm-gui`（GUI 引导页提供「以管理员身份重启」）。`switch` / `init` 均受影响。

## 卸载版本后目录还在 / 卸载失败

**原因**：版本目录被其他进程或杀毒软件占用，删除部分完成后中止。

**处理**：关闭占用该目录的进程（或加杀软排除），按提示手动删除残留目录 `store/<sdk>/<version>`。sdkm 不会留下半配置——符号链接 / PATH / `current_version` 在删除目录前已按规则处理。

## GUI 更新后还是旧版本 / 提示"重启后生效"

**原因**：磁盘上的二进制已替换，但运行中的 GUI 进程仍是旧版。

**处理**：点诊断页「立即重启」，或手动关闭 GUI 重新打开。重启前 CLI/GUI 版本不一致告警属预期，会自动消失。

## 磁盘占用变大 / 想清理临时文件

```bash
sdkm clean            # 清 .cache + .tmp（不动已安装 SDK）
sdkm clean --cache    # 只清版本缓存
sdkm clean --tmp      # 只清下载与解压残留
```

安装失败后想重来，先 `sdkm clean` 再重试，比手动找目录安全。

## WSL 里 `sdkm` 行为怪异 / 改的不是 Linux 的环境

**原因**：WSL 默认继承 Windows PATH（`appendWindowsPath=true`），但 bash 不会自动补 `.exe` 后缀——裸 `sdkm` 只命中 Linux 版；若你敲的是 `sdkm.exe`，跑的是 **Windows sdkm**，它操作的是 Windows home、注册表与 PowerShell profile，完全不碰 Linux。

**处理**：

```bash
command -v sdkm
file "$(command -v sdkm)"    # ELF = Linux 版；PE32+ = Windows 版
```

要管理 Linux 环境就装 Linux 版并用裸 `sdkm`；两者互不影响（各自独立的 home）。

## 怎么彻底卸载 sdkm

```bash
sdkm self uninstall    # 清理所有受管 SDK 的激活环境、当前 shell profile 里的 hook 行、home 目录内容
```

二进制本身需手动删除（运行中的程序跨平台无法可靠自删）。详见 [commands.md · self uninstall](./commands.md#sdkm-self-uninstall)。GUI 用户可在诊断页「高级操作 → 卸载 sdkm」。

## 还解决不了？

1. `sdkm doctor` 输出整份报告；
2. `sdkm --version` 与操作系统版本；
3. 复现命令与完整报错；
4. 提到 [GitHub Issues](https://github.com/borenchan/sdkmate/issues)。