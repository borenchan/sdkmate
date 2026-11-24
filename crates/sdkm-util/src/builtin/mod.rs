// 内置 SDK 注册表（统一种子源）
//
// 每个内置 SDK 一条 `SdkSeed`：描述它的版本源、下载模板、二进制布局等静态属性。
// init 时物化为 config.toml 的 [[sdk]] 条目（`ensure_builtin_sdks` 负责增量补全），
// 运行时除 `Sdk::Built` 专属路径（Java 两步查询、下载模板分支）外统一读 config。
//
// 三类远程接入形态（见 docs/custom-sdk.md；另有本地 switch-only，不产生内置种子）：
// - 标准 A（模板直链）：version_url 返回版本列表 JSON + download_url 占位符模板
// - 标准 B（GitHub Releases）：version_url 指向 GH releases API，引擎自动解析版本
//   并从 assets 中挑选当前平台的压缩包直链（download_url 留空即可）
// - 组合模式：GH 版本列表 + 官方 CDN 模板下载（GH release 不含二进制资产时）
//
// 文件组织：本模块只放类型、构造函数与聚合；种子按四类拆分在子模块：
// - lang       语言 SDK / Runtime
// - toolchain  开发工具链
// - cli        开发辅助 CLI
// - middleware 归档式中间件 / 服务器软件
//
// 各类标准：
// - 标准 B 一律用 `SdkSeed::gh()`；资产前缀与 SDK 名不同时用 `gh_prefixed()`
// - 组合模式一律用 `SdkSeed::gh_cdn()`
// - 标准 A 与专属路径 SDK 用结构体字面量（各运行时的版本/下载语义不同）
//
// 内置准入门槛：语言/数据库/中间件优先，star ≥ 15k 或 mega-famous 级常用度（太冷门无内置价值）；
// 资产命名与解压布局须满足标准 B 引擎（装完即用：主命令直接命中 PATH——
// zip 内 exe 带平台名的除名，如 codex/yq；裸二进制无压缩包的除名，如 jq/mkcert）。

mod cli;
mod lang;
mod middleware;
mod toolchain;

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
    /// GH 资产名前缀门（None = 取 primary_executables[0]；标准 A / 专属路径种子留 None，
    /// 标准 B 由构造函数填 SDK 名，少数前缀不符的用 gh_prefixed 覆盖）
    pub asset_prefix: Option<&'static str>,
    /// 额外环境变量种子（值支持 {sdk_dir} 等占位符）
    pub extra_vars: &'static [(&'static str, &'static str)],
    /// 额外 PATH 条目（相对 symlink 目录，如 Windows Python 的 Scripts）
    pub extra_paths: &'static [&'static str],
    /// 是否把 bin 目录加入 PATH（false = 仅设 extra_vars，不注入 PATH，如 Tomcat 的 CATALINA_HOME）
    pub add_to_path: bool,
}

impl SdkSeed {
    /// 标准 B（GitHub Releases 直链）构造，覆盖绝大多数 GH 分发工具。
    ///
    /// 默认：根布局、资产前缀 = SDK 名、Default os/arch 风格、主命令直接进 PATH。
    /// - 二进制在 `bin/` 子目录：`.bin("bin")`
    /// - 资产前缀与 SDK 名不同：改用 `gh_prefixed()`
    /// - GH release 不含二进制资产：改用 `gh_cdn()`
    pub const fn gh(
        name: &'static str,
        version_url: &'static str,
        primary_executables: &'static [&'static str],
    ) -> Self {
        SdkSeed {
            name,
            variant: None,
            version_url,
            version_fallback_url: None,
            download_url: None,
            download_fallback_url: None,
            bin_dir: None,
            os_style: OsStyle::Default,
            arch_style: ArchStyle::Default,
            primary_executables,
            // 资产前缀默认取 SDK 名：GH 资产普遍以项目名命名，主命令名可能不同（ripgrep→rg）
            asset_prefix: Some(name),
            extra_vars: &[],
            extra_paths: &[],
            add_to_path: true,
        }
    }

    /// 标准 B 特例：资产名前缀与 SDK 名不同（如 claude-code 资产是 `claude-*`、
    /// duckdb 资产是 `duckdb_cli-*`）。
    pub const fn gh_prefixed(
        name: &'static str,
        version_url: &'static str,
        asset_prefix: &'static str,
        primary_executables: &'static [&'static str],
    ) -> Self {
        let mut seed = Self::gh(name, version_url, primary_executables);
        seed.asset_prefix = Some(asset_prefix);
        seed
    }

    /// 组合模式：GH 版本列表 + 官方 CDN 下载模板。
    ///
    /// GH release 只有 tag 没有二进制资产，版本走 GH API、下载走官方 CDN；
    /// 官方 CDN 的 arch 命名统一 Go 风格（amd64/arm64），os 为 Default。
    pub const fn gh_cdn(
        name: &'static str,
        version_url: &'static str,
        download_url: &'static str,
        primary_executables: &'static [&'static str],
    ) -> Self {
        SdkSeed {
            name,
            variant: None,
            version_url,
            version_fallback_url: None,
            download_url: Some(download_url),
            download_fallback_url: None,
            bin_dir: None,
            os_style: OsStyle::Default,
            arch_style: ArchStyle::Go,
            primary_executables,
            asset_prefix: Some(name),
            extra_vars: &[],
            extra_paths: &[],
            add_to_path: true,
        }
    }

    /// 覆盖 bin_dir（解压后二进制在子目录，如 cmake/gh 的 "bin"）
    pub const fn bin(mut self, dir: &'static str) -> Self {
        self.bin_dir = Some(dir);
        self
    }
}

/// 内置 SDK 注册表：按四类分组（唯一事实来源：init 种子、is_builtin 保护、布局默认值均出自此表）。
/// 新增种子只改对应分类文件的 `SEEDS`，本表仅在新增分类时改动。
pub static SDK_SEEDS: &[&[&SdkSeed]] = &[lang::SEEDS, toolchain::SEEDS, cli::SEEDS, middleware::SEEDS];

/// 按分类顺序展开全部内置种子
pub fn all_seeds() -> impl Iterator<Item = &'static SdkSeed> {
    SDK_SEEDS.iter().flat_map(|group| group.iter().copied())
}

/// 按名字查找内置种子
pub fn find_seed(name: &str) -> Option<&'static SdkSeed> {
    all_seeds().find(|s| s.name == name)
}

/// 按 BuiltinSdk 变体查找内置种子（仅专属路径 SDK 有 variant）
pub fn find_seed_by_variant(sdk: &BuiltinSdk) -> Option<&'static SdkSeed> {
    all_seeds().find(|s| s.variant.as_ref().is_some_and(|v| v == sdk))
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// 注册表不变量：名字唯一、主命令非空、可远程安装、GH 直链有资产前缀、按名可查。
    #[test]
    fn seeds_are_well_formed() {
        let mut names = HashSet::new();
        for seed in all_seeds() {
            assert!(names.insert(seed.name), "duplicate seed name: {}", seed.name);
            assert!(!seed.primary_executables.is_empty(), "{} has no primary executable", seed.name);
            assert!(
                !seed.version_url.is_empty() || seed.download_url.is_some(),
                "{} is not installable remotely",
                seed.name
            );
            // 标准 B 靠资产前缀门挑平台包；空前缀会匹配到任意资产
            if seed.download_url.is_none() {
                let prefix = seed.asset_prefix.unwrap_or(seed.primary_executables[0]);
                assert!(!prefix.is_empty(), "{} has empty asset prefix", seed.name);
            }
            assert_eq!(find_seed(seed.name).map(|s| s.name), Some(seed.name));
        }
    }

    /// 专属路径 SDK 的 variant 唯一（find_seed_by_variant 只返回首个命中）。
    #[test]
    fn variants_are_unique() {
        for seed in all_seeds() {
            if let Some(variant) = &seed.variant {
                assert_eq!(find_seed_by_variant(variant).map(|s| s.name), Some(seed.name));
            }
        }
    }
}
