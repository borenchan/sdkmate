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
//
// 文件组织：本模块只放类型、构造函数与聚合；种子按领域拆分在子模块（core/cli/devops/security）。
//
// 内置准入门槛：GH star ≥ 5k 或同等知名度（太冷门无内置价值）；资产命名与解压布局须
// 满足标准 B 引擎（装完即用：主命令直接命中 PATH——zip 内 exe 带平台名的除名，如 codex/yq）。

mod cli;
mod core;
mod devops;
mod security;

use crate::config_helper::{ArchStyle, OsStyle};
use crate::sdk::BuiltinSdk;

/// 内置 SDK 注册条目
#[derive(Debug)]
pub struct SdkSeed {
    /// SDK 名（config.toml 中的唯一标识，同时用于 is_builtin_sdk 保护）
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

impl SdkSeed {
    /// 标准 B（GitHub Releases 直链）种子的快捷构造，覆盖绝大多数 GH 分发工具：
    /// 版本源 = `api.github.com/repos/{repo}/releases`、资产直链下载、`bin_dir`/`asset_prefix` 可选、
    /// Default os/arch 风格、无额外变量。特例（模板下载/非 Default 风格/专属路径）用结构体字面量。
    pub const fn gh(
        name: &'static str,
        version_url: &'static str,
        primary_executables: &'static [&'static str],
        bin_dir: Option<&'static str>,
        asset_prefix: Option<&'static str>,
    ) -> Self {
        SdkSeed {
            name,
            variant: None,
            version_url,
            version_fallback_url: None,
            download_url: None,
            download_fallback_url: None,
            bin_dir,
            os_style: OsStyle::Default,
            arch_style: ArchStyle::Default,
            primary_executables,
            asset_prefix,
            extra_vars: &[],
            extra_paths: &[],
        }
    }
}

/// Java 的 assets 详情 API 模板（两步查询专用，其余 SDK 不使用）
pub const JAVA_ASSETS_URL: &str = "https://api.adoptium.net/v3/assets/latest/{feature_version}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse";

/// 内置 SDK 注册表（唯一事实来源：init 种子、is_builtin 保护、布局默认值均出自此表）
pub static SDK_SEEDS: &[&SdkSeed] = &[
    // ── 专属路径 SDK（Sdk::Built，保留历史解析/下载逻辑）─────────────
    &core::JAVA,
    &core::MAVEN,
    &core::NODE,
    &core::PYTHON,
    &core::GO,
    // ── 引擎型 SDK（标准 A/B 数据驱动，无专属代码路径）────────────────
    &core::BUN,
    &core::PNPM,
    &core::DENO,
    &core::UV,
    &core::CLAUDE_CODE,
    &core::CMAKE,
    &core::GH,
    &devops::HELM,
    &devops::TERRAFORM,
    // ── 第二波（分类扩容）──────────────────────────────────────────
    &cli::FZF,
    &cli::RIPGREP,
    &cli::FD,
    &cli::BAT,
    &cli::EZA,
    &cli::DELTA,
    &devops::JUST,
    &devops::TASK,
    &devops::GOLANGCI_LINT,
    &devops::WATCHEXEC,
    &devops::LAZYGIT,
    &devops::LAZYDOCKER,
    &devops::K9S,
    &devops::STERN,
    &devops::HELMFILE,
    &devops::DIVE,
    &devops::GRPCURL,
    &devops::TEMPORAL,
    &security::AGE,
];

/// 按名字查找内置种子
pub fn find_seed(name: &str) -> Option<&'static SdkSeed> {
    SDK_SEEDS.iter().copied().find(|s| s.name == name)
}

/// 按 BuiltinSdk 变体查找内置种子（仅专属路径 SDK 有 variant）
pub fn find_seed_by_variant(sdk: &BuiltinSdk) -> Option<&'static SdkSeed> {
    SDK_SEEDS.iter().copied().find(|s| s.variant.as_ref().is_some_and(|v| v == sdk))
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
