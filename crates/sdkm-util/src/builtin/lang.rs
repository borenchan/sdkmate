// 语言 SDK / Runtime：版本发现与下载形态各异——专属路径 SDK 用结构体字面量，
// deno 走标准 B（GitHub Releases 直链）。

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
    add_to_path: true,
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
    add_to_path: true,
};

pub const PYTHON: SdkSeed = SdkSeed {
    name: "python",
    variant: Some(BuiltinSdk::Python),
    // 主源：uv 维护的元数据（raw.githubusercontent 无速率限制）；备源：GH Releases API
    // 2026-10 起文件从 crates/uv-python/ 迁至 crates/uv-python-managed/
    version_url: "https://raw.githubusercontent.com/astral-sh/uv/main/crates/uv-python-managed/download-metadata.json",
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
    add_to_path: true,
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
    add_to_path: true,
};

pub const DENO: SdkSeed = SdkSeed::gh("deno", "https://api.github.com/repos/denoland/deno/releases", &["deno"]);

/// 语言 SDK / Runtime 分类的全部种子（新增语言运行时在这里追加）
pub static SEEDS: &[&SdkSeed] = &[&JAVA, &NODE, &PYTHON, &GO, &DENO];
