// 内置 SDK 注册表（统一种子源）
//
// 每个内置 SDK 一条 `SdkSeed`：描述它的版本源、下载模板、二进制布局等静态属性。
// init 时物化为 config.toml 的 [[sdk]] 条目（`ensure_builtin_sdks` 负责增量补全），
// 运行时除 `Sdk::Built` 专属路径（Java 两步查询、下载模板分支）外统一读 config。
//
// 两类接入标准（见 docs/custom-sdk.md）：
// - 标准 A（模板直链）：version_url 返回版本列表 JSON + download_url 占位符模板
// - 标准 B（GitHub Releases）：version_url 指向 GH releases API，引擎自动解析版本
//   并从 assets 中挑选当前平台的压缩包直链（download_url 留空即可）
// - 组合模式：GH 版本列表 + 官方 CDN 模板下载（helm/terraform——GH release 不含二进制资产）

use crate::config_helper::{ArchStyle, OsStyle};
use crate::sdk::BuiltinSdk;

/// 内置 SDK 注册条目
pub struct SdkSeed {
    /// SDK 名（config.toml 中的唯一标识，同时用于 is_builtin_sdk 保护与 GH 资产前缀打分）
    pub name: &'static str,
    /// 对应 BuiltinSdk 枚举变体；None = 引擎型 SDK（无专属代码路径，全走通用引擎）
    pub variant: Option<BuiltinSdk>,
    /// 版本发现主源 URL（空串 = 无远程版本发现，仅精确版本安装）
    pub version_url: &'static str,
    /// 版本发现备源 URL（主源失败时回退）
    pub version_fallback_url: Option<&'static str>,
    /// 下载主源 URL 模板（None = 无模板，仅靠版本源直链；含 {version}/{os}/{arch}/{ext} 等占位符）
    pub download_url: Option<&'static str>,
    /// 下载备源模板（下载主源失败时回退）
    pub download_fallback_url: Option<&'static str>,
    /// 二进制子目录（None = 解压后二进制在 SDK 根目录，如 Windows Node、bun）
    pub bin_dir: Option<&'static str>,
    /// 下载模板的 os 命名风格（占位符 {os} 的取值映射）
    pub os_style: OsStyle,
    /// 下载模板的 arch 命名风格（占位符 {arch} 的取值映射）
    pub arch_style: ArchStyle,
    /// 主可执行文件（PATH 冲突检测用，不含扩展名）
    pub primary_executables: &'static [&'static str],
    /// GH 资产名前缀门（None = 取 primary_executables[0]；资产前缀与主命令名不同的工具需显式指定，如 ripgrep→"ripgrep"）
    pub asset_prefix: Option<&'static str>,
    /// 额外环境变量种子（值支持 {sdk_dir} 等占位符）
    pub extra_vars: &'static [(&'static str, &'static str)],
    /// 额外 PATH 条目（相对 symlink 目录，如 Windows Python 的 Scripts）
    pub extra_paths: &'static [&'static str],
}

// ─── 注册表 ─────────────────────────────────────────────────────
//

/// Java 的 assets 详情 API 模板（两步查询专用，其余 SDK 不使用）
pub const JAVA_ASSETS_URL: &str = "https://api.adoptium.net/v3/assets/latest/{feature_version}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse";

/// 内置 SDK 注册表（唯一事实来源：init 种子、is_builtin 保护、布局默认值均出自此表）
pub const SDK_SEEDS: &[SdkSeed] = &[
    // ── 专属路径 SDK（Sdk::Built，保留历史解析/下载逻辑）─────────────
    SdkSeed {
        name: "java",
        variant: Some(BuiltinSdk::Java),
        version_url: "https://api.adoptium.net/v3/info/available_releases",
        version_fallback_url: None,
        // {feature_version} = 大版本号（21/17/11）；{os}/{arch} = Adoptium 映射
        download_url: Some(
            "https://api.adoptium.net/v3/binary/latest/{feature_version}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
        ),
        download_fallback_url: None,
        bin_dir: Some("bin"),
        os_style: OsStyle::Adoptium,
        arch_style: ArchStyle::Adoptium,
        primary_executables: &["java", "javac"],
        asset_prefix: None,
        extra_vars: &[("JAVA_HOME", "{sdk_dir}")],
        extra_paths: &[],
    },
    SdkSeed {
        name: "maven",
        variant: Some(BuiltinSdk::Maven),
        version_url: "", // 无远程版本发现，仅精确版本安装
        version_fallback_url: None,
        // {version} = 3.9.9；{ext} = zip(win)/tar.gz(unix)
        download_url: Some(
            "https://dlcdn.apache.org/maven/maven-3/{version}/binaries/apache-maven-{version}-bin.{ext}",
        ),
        download_fallback_url: None,
        bin_dir: Some("bin"),
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["mvn"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "node",
        variant: Some(BuiltinSdk::Node),
        version_url: "https://nodejs.org/dist/index.json",
        version_fallback_url: None,
        // {version} = v20.11.0（VPrefixed）；{os} = Short 映射（win/darwin/linux）
        download_url: Some("https://nodejs.org/dist/{version}/node-{version}-{os}-{arch}.{ext}"),
        download_fallback_url: None,
        // Windows zip 扁平（node.exe 在根）；Unix tar.gz 有 bin/
        bin_dir: if cfg!(target_os = "windows") { None } else { Some("bin") },
        os_style: OsStyle::Short,
        arch_style: ArchStyle::Default,
        primary_executables: &["node", "npm"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "python",
        variant: Some(BuiltinSdk::Python),
        // 主源：uv 维护的元数据（raw.githubusercontent 无速率限制）；备源：GH Releases API
        version_url: "https://raw.githubusercontent.com/astral-sh/uv/main/crates/uv-python/download-metadata.json",
        version_fallback_url: Some("https://api.github.com/repos/astral-sh/python-build-standalone/releases"),
        // {version}/{release_tag}/{platform} = 动态发现填充
        download_url: Some(
            "https://github.com/astral-sh/python-build-standalone/releases/download/{release_tag}/cpython-{version}%2B{release_tag}-{platform}-install_only.tar.gz",
        ),
        download_fallback_url: None,
        // install_only 双层提升后：Windows 扁平，Unix 有 bin/
        bin_dir: if cfg!(target_os = "windows") { None } else { Some("bin") },
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Python,
        primary_executables: &["python", "python3"],
        asset_prefix: None,
        extra_vars: &[],
        // Windows 的 pip 在 Scripts 子目录
        extra_paths: if cfg!(target_os = "windows") { &["Scripts"] } else { &[] },
    },
    SdkSeed {
        name: "go",
        variant: Some(BuiltinSdk::Go),
        // 官方全版本 JSON（含 sha256/size/files）
        version_url: "https://go.dev/dl/?mode=json&include=all",
        version_fallback_url: None,
        // {version} = 1.26.5（不含 go 前缀，模板组合 go{version}）；{arch} = Go 映射（amd64/arm64/386）
        download_url: Some("https://go.dev/dl/go{version}.{os}-{arch}.{ext}"),
        // 国内加速备源：Google 中国 CDN
        download_fallback_url: Some("https://golang.google.cn/dl/go{version}.{os}-{arch}.{ext}"),
        bin_dir: Some("bin"),
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Go,
        primary_executables: &["go", "gofmt"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    // ── 引擎型 SDK（标准 A/B 数据驱动，无专属代码路径）────────────────
    SdkSeed {
        name: "bun",
        variant: None,
        version_url: "https://api.github.com/repos/oven-sh/bun/releases",
        version_fallback_url: None,
        download_url: None, // GH 直链
        download_fallback_url: None,
        // zip 顶层 bun-windows-x64/ 提升后 bun.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["bun"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "pnpm",
        variant: None,
        version_url: "https://api.github.com/repos/pnpm/pnpm/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 pnpm.exe（实测，win32 词表由引擎同义词覆盖）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["pnpm"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "deno",
        variant: None,
        version_url: "https://api.github.com/repos/denoland/deno/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 裸 deno.exe 单文件（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["deno"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "uv",
        variant: None,
        version_url: "https://api.github.com/repos/astral-sh/uv/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // win 根 3 个 exe；linux 顶层目录提升后 uv/uvx 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["uv", "uvx"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "claude-code",
        variant: None,
        version_url: "https://api.github.com/repos/anthropics/claude-code/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // win zip 裸 claude.exe / linux tar 裸 claude（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["claude"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "cmake",
        variant: None,
        version_url: "https://api.github.com/repos/Kitware/CMake/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层 cmake-x.y.z-{os}-{arch}/ 提升后 bin/ 保留（实测）
        bin_dir: Some("bin"),
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["cmake"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "gh",
        variant: None,
        version_url: "https://api.github.com/repos/cli/cli/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 内 bin/gh.exe，顶层单目录提升后 bin/ 保留（实测）
        bin_dir: Some("bin"),
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["gh"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "helm",
        variant: None,
        // GH release v4.x 仅签名文件（无二进制资产）→ 引擎无直链 → 落到官方 CDN 模板
        version_url: "https://api.github.com/repos/helm/helm/releases",
        version_fallback_url: None,
        // {version} = 4.2.4（模板组合 v{version}）；{os} = Default（windows/linux/darwin）；{arch} = Go 映射（amd64/arm64）
        download_url: Some("https://get.helm.sh/helm-v{version}-{os}-{arch}.{ext}"),
        download_fallback_url: None,
        // tar.gz 顶层 {os}-{arch}/ 提升后 helm 在根
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Go,
        primary_executables: &["helm"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "terraform",
        variant: None,
        // GH release 仅 tag 无二进制资产 → 引擎无直链 → 落到 hashicorp 官方模板
        version_url: "https://api.github.com/repos/hashicorp/terraform/releases",
        version_fallback_url: None,
        // 全平台 zip，不用 {ext}；{arch} = Go 映射（amd64/arm64/386）
        download_url: Some("https://releases.hashicorp.com/terraform/{version}/terraform_{version}_{os}-{arch}.zip"),
        download_fallback_url: None,
        // zip 裸 terraform.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Go,
        primary_executables: &["terraform"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    // ── 第二波（分类扩容）：前端/后端/中间件/运维/安全 ───────────────
    // （yq 候选除名：zip 内主 exe 带平台名 yq_windows_amd64.exe，需重命名能力，同 codex）
    SdkSeed {
        name: "fzf",
        variant: None,
        version_url: "https://api.github.com/repos/junegunn/fzf/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 fzf.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["fzf"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "ripgrep",
        variant: None,
        version_url: "https://api.github.com/repos/BurntSushi/ripgrep/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 rg.exe 在根（实测）；资产前缀与主命令不同
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["rg"],
        asset_prefix: Some("ripgrep"),
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "fd",
        variant: None,
        version_url: "https://api.github.com/repos/sharkdp/fd/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 fd.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["fd"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "bat",
        variant: None,
        version_url: "https://api.github.com/repos/sharkdp/bat/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 bat.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["bat"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "eza",
        variant: None,
        version_url: "https://api.github.com/repos/eza-community/eza/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 eza.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["eza"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "delta",
        variant: None,
        version_url: "https://api.github.com/repos/dandavison/delta/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 delta.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["delta"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "just",
        variant: None,
        version_url: "https://api.github.com/repos/casey/just/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 just.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["just"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "task",
        variant: None,
        version_url: "https://api.github.com/repos/go-task/task/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 task.exe（实测，nightly tag 由 prerelease 过滤）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["task"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "golangci-lint",
        variant: None,
        version_url: "https://api.github.com/repos/golangci/golangci-lint/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 golangci-lint.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["golangci-lint"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "watchexec",
        variant: None,
        version_url: "https://api.github.com/repos/watchexec/watchexec/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // 顶层目录提升后 watchexec.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["watchexec"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "lazygit",
        variant: None,
        version_url: "https://api.github.com/repos/jesseduffield/lazygit/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 lazygit.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["lazygit"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "lazydocker",
        variant: None,
        version_url: "https://api.github.com/repos/jesseduffield/lazydocker/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 lazydocker.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["lazydocker"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "k9s",
        variant: None,
        version_url: "https://api.github.com/repos/derailed/k9s/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 k9s.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["k9s"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "stern",
        variant: None,
        version_url: "https://api.github.com/repos/stern/stern/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 stern.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["stern"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "helmfile",
        variant: None,
        version_url: "https://api.github.com/repos/helmfile/helmfile/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // tar.gz 根 helmfile.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["helmfile"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "dive",
        variant: None,
        version_url: "https://api.github.com/repos/wagoodman/dive/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 dive.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["dive"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "grpcurl",
        variant: None,
        version_url: "https://api.github.com/repos/fullstorydev/grpcurl/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根 grpcurl.exe（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["grpcurl"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "temporal",
        variant: None,
        version_url: "https://api.github.com/repos/temporalio/temporal/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 根多 exe：temporal-server 等（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["temporal-server"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
    SdkSeed {
        name: "age",
        variant: None,
        version_url: "https://api.github.com/repos/FiloSottile/age/releases",
        version_fallback_url: None,
        download_url: None,
        download_fallback_url: None,
        // zip 内 age/ 提升后 age.exe + age-keygen.exe 在根（实测）
        bin_dir: None,
        os_style: OsStyle::Default,
        arch_style: ArchStyle::Default,
        primary_executables: &["age", "age-keygen"],
        asset_prefix: None,
        extra_vars: &[],
        extra_paths: &[],
    },
];

/// 按名字查找内置种子
pub fn find_seed(name: &str) -> Option<&'static SdkSeed> {
    SDK_SEEDS.iter().find(|s| s.name == name)
}

/// 按 BuiltinSdk 变体查找内置种子（仅专属路径 SDK 有 variant）
pub fn find_seed_by_variant(sdk: &BuiltinSdk) -> Option<&'static SdkSeed> {
    SDK_SEEDS.iter().find(|s| s.variant.as_ref().is_some_and(|v| v == sdk))
}

/// 名字是否为内置 SDK（remove-sdk / 字段删除保护）
pub fn is_builtin_sdk(name: &str) -> bool {
    find_seed(name).is_some()
}

/// 查 SDK 的主可执行文件（PATH 冲突检测；内置与自定义一律支持，自定义按名字猜 <name> 本身）
pub fn primary_executables_for(name: &str) -> Vec<String> {
    match find_seed(name) {
        Some(seed) => seed.primary_executables.iter().map(|s| s.to_string()).collect(),
        None => vec![name.to_string()],
    }
}
