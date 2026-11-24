// 归档式中间件 / 服务器软件：只托管"解压即用、自己启动"的归档版，不负责服务注册与编排。
// 标准 B 用 SdkSeed::gh()；hashicorp 系组合模式用 SdkSeed::gh_cdn()；
// Tomcat 是专属路径 + HTML 目录索引的标准 A，用结构体字面量。

use super::SdkSeed;
use crate::config_helper::{ArchStyle, OsStyle};
use crate::sdk::BuiltinSdk;

pub const TOMCAT: SdkSeed = SdkSeed {
    name: "tomcat",
    variant: Some(BuiltinSdk::Tomcat),
    // Apache 归档根索引（列 tomcat-9/10/11 分支目录）；版本列表经多请求逐分支解析
    version_url: "https://archive.apache.org/dist/tomcat/",
    version_fallback_url: Some("https://dlcdn.apache.org/tomcat/"),
    // {feature_version} = 主版本号（9/10/11）；{ext} = zip(win)/tar.gz(unix)
    // 主源 dlcdn（当前版本 CDN，快；历史版本 404 时不重试，立即回退 archive）
    download_url: Some(
        "https://dlcdn.apache.org/tomcat/tomcat-{feature_version}/v{version}/bin/apache-tomcat-{version}.{ext}",
    ),
    download_fallback_url: Some(
        "https://archive.apache.org/dist/tomcat/tomcat-{feature_version}/v{version}/bin/apache-tomcat-{version}.{ext}",
    ),
    bin_dir: Some("bin"),
    os_style: OsStyle::Default,
    arch_style: ArchStyle::Default,
    primary_executables: &["catalina"],
    asset_prefix: None,
    extra_vars: &[("CATALINA_HOME", "{sdk_dir}")],
    extra_paths: &[],
    // bin/ 下 startup/version 等脚本名太通用，不入 PATH，仅设 CATALINA_HOME
    add_to_path: false,
};

// ── Web 服务器 / 反向代理 ────────────────────────────────────────

// buildable-artifact 源码包被未识别段排除
pub const CADDY: SdkSeed = SdkSeed::gh("caddy", "https://api.github.com/repos/caddyserver/caddy/releases", &["caddy"]);
// .src.tar.gz 源码资产被 "src" 未识别段排除
pub const TRAEFIK: SdkSeed =
    SdkSeed::gh("traefik", "https://api.github.com/repos/traefik/traefik/releases", &["traefik"]);

// ── KV 存储 / 数据库 ─────────────────────────────────────────────

// zip 顶层目录提升后 etcd.exe + etcdctl.exe（+ etcdutl.exe）三命令在根
pub const ETCD: SdkSeed = SdkSeed::gh(
    "etcd",
    "https://api.github.com/repos/etcd-io/etcd/releases",
    &["etcd", "etcdctl"],
);
// zip 根 duckdb.exe；libduckdb-*（C 库发行包）被前缀门排除，osx-universal 资产由 universal 段识别
pub const DUCKDB: SdkSeed = SdkSeed::gh_prefixed(
    "duckdb",
    "https://api.github.com/repos/duckdb/duckdb/releases",
    "duckdb_cli",
    &["duckdb"],
);

// ── 基础设施（组合模式：GH 版本列表 + hashicorp 官方 CDN 模板）───
// hashicorp 系 GH release 仅 tag 无二进制资产 → 引擎无直链 → 落到官方模板

pub const CONSUL: SdkSeed = SdkSeed::gh_cdn(
    "consul",
    "https://api.github.com/repos/hashicorp/consul/releases",
    // 全平台 zip，不用 {ext}；{arch} = Go 映射（amd64/arm64）
    "https://releases.hashicorp.com/consul/{version}/consul_{version}_{os}-{arch}.zip",
    &["consul"],
);

pub const VAULT: SdkSeed = SdkSeed::gh_cdn(
    "vault",
    "https://api.github.com/repos/hashicorp/vault/releases",
    "https://releases.hashicorp.com/vault/{version}/vault_{version}_{os}-{arch}.zip",
    &["vault"],
);

/// 归档式中间件 / 服务器软件分类的全部种子（新增中间件在这里追加）
pub static SEEDS: &[&SdkSeed] = &[&TOMCAT, &CADDY, &TRAEFIK, &ETCD, &CONSUL, &VAULT, &DUCKDB];
