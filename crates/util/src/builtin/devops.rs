// 开发运维类种子：构建任务/后端 lint/终端 UI/容器/K8s/基础设施（组合模式）

use super::SdkSeed;
use crate::config_helper::{ArchStyle, OsStyle};

// ── 构建 & 任务 ──────────────────────────────────────────────────

pub const JUST: SdkSeed = SdkSeed::gh("just", "https://api.github.com/repos/casey/just/releases", &["just"]);
// nightly tag 由 prerelease 过滤
pub const TASK: SdkSeed = SdkSeed::gh("task", "https://api.github.com/repos/go-task/task/releases", &["task"]);
pub const GOLANGCI_LINT: SdkSeed = SdkSeed::gh(
    "golangci-lint",
    "https://api.github.com/repos/golangci/golangci-lint/releases",
    &["golangci-lint"],
);
pub const WATCHEXEC: SdkSeed = SdkSeed::gh(
    "watchexec",
    "https://api.github.com/repos/watchexec/watchexec/releases",
    &["watchexec"],
);

// ── 终端 UI ──────────────────────────────────────────────────────

pub const LAZYGIT: SdkSeed = SdkSeed::gh(
    "lazygit",
    "https://api.github.com/repos/jesseduffield/lazygit/releases",
    &["lazygit"],
);
pub const LAZYDOCKER: SdkSeed = SdkSeed::gh(
    "lazydocker",
    "https://api.github.com/repos/jesseduffield/lazydocker/releases",
    &["lazydocker"],
);
pub const K9S: SdkSeed = SdkSeed::gh("k9s", "https://api.github.com/repos/derailed/k9s/releases", &["k9s"]);
pub const STERN: SdkSeed = SdkSeed::gh("stern", "https://api.github.com/repos/stern/stern/releases", &["stern"]);
pub const HELMFILE: SdkSeed = SdkSeed::gh(
    "helmfile",
    "https://api.github.com/repos/helmfile/helmfile/releases",
    &["helmfile"],
);
pub const DIVE: SdkSeed = SdkSeed::gh("dive", "https://api.github.com/repos/wagoodman/dive/releases", &["dive"]);
pub const GRPCURL: SdkSeed = SdkSeed::gh(
    "grpcurl",
    "https://api.github.com/repos/fullstorydev/grpcurl/releases",
    &["grpcurl"],
);
// zip 根多 exe：temporal-server 等
pub const TEMPORAL: SdkSeed = SdkSeed::gh(
    "temporal",
    "https://api.github.com/repos/temporalio/temporal/releases",
    &["temporal-server"],
);

// ── 基础设施（组合模式：GH 版本列表 + 官方 CDN 模板）──────────────

pub const HELM: SdkSeed = SdkSeed {
    name: "helm",
    variant: None,
    // GH release v4.x 仅签名文件（无二进制资产）→ 引擎无直链 → 落到官方 CDN 模板
    version_url: "https://api.github.com/repos/helm/helm/releases",
    version_fallback_url: None,
    // {version} = 4.2.4（模板组合 v{version}）；{os} = Default；{arch} = Go 映射（amd64/arm64）
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
};

pub const TERRAFORM: SdkSeed = SdkSeed {
    name: "terraform",
    variant: None,
    // GH release 仅 tag 无二进制资产 → 引擎无直链 → 落到 hashicorp 官方模板
    version_url: "https://api.github.com/repos/hashicorp/terraform/releases",
    version_fallback_url: None,
    // 全平台 zip，不用 {ext}；{arch} = Go 映射（amd64/arm64/386）
    download_url: Some("https://releases.hashicorp.com/terraform/{version}/terraform_{version}_{os}-{arch}.zip"),
    download_fallback_url: None,
    // zip 裸 terraform.exe
    bin_dir: None,
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Go,
    primary_executables: &["terraform"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
};
