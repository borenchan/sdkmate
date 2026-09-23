// 核心语言与运行时种子：旧 5 个专属路径 SDK + JS/Python 生态引擎型工具

use super::SdkSeed;
use crate::config_helper::{ArchStyle, OsStyle};
use crate::sdk::BuiltinSdk;

pub const JAVA: SdkSeed = SdkSeed {
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
};

pub const MAVEN: SdkSeed = SdkSeed {
    name: "maven",
    variant: Some(BuiltinSdk::Maven),
    version_url: "", // 无远程版本发现，仅精确版本安装
    version_fallback_url: None,
    // {version} = 3.9.9；{ext} = zip(win)/tar.gz(unix)
    download_url: Some("https://dlcdn.apache.org/maven/maven-3/{version}/binaries/apache-maven-{version}-bin.{ext}"),
    download_fallback_url: None,
    bin_dir: Some("bin"),
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Default,
    primary_executables: &["mvn"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
};

pub const NODE: SdkSeed = SdkSeed {
    name: "node",
    variant: Some(BuiltinSdk::Node),
    version_url: "https://nodejs.org/dist/index.json",
    version_fallback_url: None,
    // {version} = v20.11.0（含 v 前缀，专属分支加工）；{os} = Short 映射（win/darwin/linux）
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
};

pub const PYTHON: SdkSeed = SdkSeed {
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
};

pub const GO: SdkSeed = SdkSeed {
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
};

// ── 引擎型（标准 B 直链）─────────────────────────────────────────

pub const BUN: SdkSeed = SdkSeed::gh("bun", "https://api.github.com/repos/oven-sh/bun/releases", &["bun"]);
pub const PNPM: SdkSeed = SdkSeed::gh("pnpm", "https://api.github.com/repos/pnpm/pnpm/releases", &["pnpm"]);
pub const DENO: SdkSeed = SdkSeed::gh("deno", "https://api.github.com/repos/denoland/deno/releases", &["deno"]);
pub const UV: SdkSeed = SdkSeed::gh("uv", "https://api.github.com/repos/astral-sh/uv/releases", &["uv", "uvx"]);
// 资产前缀 claude 与 sdk 名 claude-code 不同
pub const CLAUDE_CODE: SdkSeed = SdkSeed::gh(
    "claude-code",
    "https://api.github.com/repos/anthropics/claude-code/releases",
    &["claude"],
)
.prefix("claude");
// zip 顶层目录提升后 bin/ 保留
pub const CMAKE: SdkSeed =
    SdkSeed::gh("cmake", "https://api.github.com/repos/Kitware/CMake/releases", &["cmake"]).bin("bin");
// zip 内 bin/gh.exe，顶层单目录提升后 bin/ 保留
pub const GH: SdkSeed = SdkSeed::gh("gh", "https://api.github.com/repos/cli/cli/releases", &["gh"]).bin("bin");
