// 数据库与中间件类种子：嵌入式数据库 / KV 存储 / Web 服务器 / 基础设施

use super::SdkSeed;
use crate::config_helper::{ArchStyle, OsStyle};

// ── 数据库 ──────────────────────────────────────────────────────

// zip 根 duckdb.exe；libduckdb-*（C 库发行包）被前缀门排除，osx-universal 资产由 universal 段识别
pub const DUCKDB: SdkSeed =
    SdkSeed::gh("duckdb", "https://api.github.com/repos/duckdb/duckdb/releases", &["duckdb"]).prefix("duckdb_cli");

// zip 顶层目录提升后 etcd.exe + etcdctl.exe（+ etcdutl.exe）三命令在根
pub const ETCD: SdkSeed = SdkSeed::gh(
    "etcd",
    "https://api.github.com/repos/etcd-io/etcd/releases",
    &["etcd", "etcdctl"],
);

// ── Web 服务器 / 反向代理 ────────────────────────────────────────

// buildable-artifact 源码包被未识别段排除
pub const CADDY: SdkSeed = SdkSeed::gh("caddy", "https://api.github.com/repos/caddyserver/caddy/releases", &["caddy"]);
// .src.tar.gz 源码资产被 "src" 未识别段排除
pub const TRAEFIK: SdkSeed =
    SdkSeed::gh("traefik", "https://api.github.com/repos/traefik/traefik/releases", &["traefik"]);

// ── 基础设施（组合模式：GH 版本列表 + hashicorp 官方 CDN 模板）───
// hashicorp 系 GH release 仅 tag 无二进制资产 → 引擎无直链 → 落到官方模板

pub const VAULT: SdkSeed = SdkSeed {
    name: "vault",
    variant: None,
    version_url: "https://api.github.com/repos/hashicorp/vault/releases",
    version_fallback_url: None,
    // 全平台 zip，不用 {ext}；{arch} = Go 映射（amd64/arm64）
    download_url: Some("https://releases.hashicorp.com/vault/{version}/vault_{version}_{os}-{arch}.zip"),
    download_fallback_url: None,
    // zip 裸 vault.exe
    bin_dir: None,
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Go,
    primary_executables: &["vault"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
};

pub const CONSUL: SdkSeed = SdkSeed {
    name: "consul",
    variant: None,
    version_url: "https://api.github.com/repos/hashicorp/consul/releases",
    version_fallback_url: None,
    download_url: Some("https://releases.hashicorp.com/consul/{version}/consul_{version}_{os}-{arch}.zip"),
    download_fallback_url: None,
    bin_dir: None,
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Go,
    primary_executables: &["consul"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
};

pub const PACKER: SdkSeed = SdkSeed {
    name: "packer",
    variant: None,
    version_url: "https://api.github.com/repos/hashicorp/packer/releases",
    version_fallback_url: None,
    download_url: Some("https://releases.hashicorp.com/packer/{version}/packer_{version}_{os}-{arch}.zip"),
    download_fallback_url: None,
    bin_dir: None,
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Go,
    primary_executables: &["packer"],
    asset_prefix: None,
    extra_vars: &[],
    extra_paths: &[],
};
