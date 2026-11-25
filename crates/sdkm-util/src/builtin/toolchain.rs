// 开发工具链：构建、包管理、部署与 IaC。
// 标准 B 用 SdkSeed::gh()；GH release 不含二进制资产的用 SdkSeed::gh_cdn()（组合模式）；
// Maven 是 XML 版本元数据 + 模板直链，用结构体字面量。

use super::SdkSeed;
use crate::config_helper::{ArchStyle, OsStyle};
use crate::sdk::BuiltinSdk;

// ── 包管理 / 构建 ────────────────────────────────────────────────

pub const MAVEN: SdkSeed = SdkSeed {
    name: "maven",
    variant: Some(BuiltinSdk::Maven),
    // 版本发现：华为云 Maven Central 镜像的 maven-metadata.xml（国内快）；官方兜底
    version_url: "https://mirrors.huaweicloud.com/repository/maven/org/apache/maven/apache-maven/maven-metadata.xml",
    version_fallback_url: Some("https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/maven-metadata.xml"),
    // Maven Central 制品（含全部历史 patch 与 4.x RC），路径按版本号平铺
    // {version} = 3.9.9；{ext} = zip(win)/tar.gz(unix)
    download_url: Some(
        // 主源华为云 Central 镜像：国内快，且比 apache dist 镜像完整（老 patch 如 3.0.2 只有 Central 有）
        "https://mirrors.huaweicloud.com/repository/maven/org/apache/maven/apache-maven/{version}/apache-maven-{version}-bin.{ext}",
    ),
    download_fallback_url: Some(
        // 备源官方 Maven Central（与主源同源制品，历史齐全）
        "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/{version}/apache-maven-{version}-bin.{ext}",
    ),
    bin_dir: Some("bin"),
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Default,
    primary_executables: &["mvn"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
    add_to_path: true,
};

// zip 顶层目录提升后 bin/ 保留
pub const CMAKE: SdkSeed =
    SdkSeed::gh("cmake", "https://api.github.com/repos/Kitware/CMake/releases", &["cmake"]).bin("bin");

pub const UV: SdkSeed = SdkSeed::gh("uv", "https://api.github.com/repos/astral-sh/uv/releases", &["uv", "uvx"]);
pub const BUN: SdkSeed = SdkSeed::gh("bun", "https://api.github.com/repos/oven-sh/bun/releases", &["bun"]);
pub const PNPM: SdkSeed = SdkSeed::gh("pnpm", "https://api.github.com/repos/pnpm/pnpm/releases", &["pnpm"]);

// ── 基础设施（组合模式：GH 版本列表 + 官方 CDN 模板）──────────────

// GH release 仅签名文件（无二进制资产）→ 官方 CDN 模板
pub const HELM: SdkSeed = SdkSeed::gh_cdn(
    "helm",
    "https://api.github.com/repos/helm/helm/releases",
    // {version} = 4.2.4（模板组合 v{version}）；{arch} = Go 映射（amd64/arm64）
    "https://get.helm.sh/helm-v{version}-{os}-{arch}.{ext}",
    &["helm"],
);

// GH release 仅 tag 无二进制资产 → hashicorp 官方模板
pub const TERRAFORM: SdkSeed = SdkSeed::gh_cdn(
    "terraform",
    "https://api.github.com/repos/hashicorp/terraform/releases",
    // 全平台 zip，不用 {ext}；{arch} = Go 映射（amd64/arm64/386）
    "https://releases.hashicorp.com/terraform/{version}/terraform_{version}_{os}-{arch}.zip",
    &["terraform"],
);

pub const PACKER: SdkSeed = SdkSeed::gh_cdn(
    "packer",
    "https://api.github.com/repos/hashicorp/packer/releases",
    "https://releases.hashicorp.com/packer/{version}/packer_{version}_{os}-{arch}.zip",
    &["packer"],
);

// ── 质量 / 任务 ──────────────────────────────────────────────────

pub const GOLANGCI_LINT: SdkSeed = SdkSeed::gh(
    "golangci-lint",
    "https://api.github.com/repos/golangci/golangci-lint/releases",
    &["golangci-lint"],
);
pub const JUST: SdkSeed = SdkSeed::gh("just", "https://api.github.com/repos/casey/just/releases", &["just"]);
// nightly tag 由 prerelease 过滤
pub const TASK: SdkSeed = SdkSeed::gh("task", "https://api.github.com/repos/go-task/task/releases", &["task"]);
pub const WATCHEXEC: SdkSeed = SdkSeed::gh(
    "watchexec",
    "https://api.github.com/repos/watchexec/watchexec/releases",
    &["watchexec"],
);
pub const HELMFILE: SdkSeed = SdkSeed::gh(
    "helmfile",
    "https://api.github.com/repos/helmfile/helmfile/releases",
    &["helmfile"],
);

/// 开发工具链分类的全部种子（新增工具链在这里追加）
pub static SEEDS: &[&SdkSeed] = &[
    &MAVEN,
    &CMAKE,
    &UV,
    &BUN,
    &PNPM,
    &HELM,
    &TERRAFORM,
    &PACKER,
    &GOLANGCI_LINT,
    &JUST,
    &TASK,
    &WATCHEXEC,
    &HELMFILE,
];
