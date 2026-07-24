use crate::sdk::BuiltinSdk;

/// SDK source config — version/download 主备 URL + 下载模板
pub struct SdkSourceConfig {
    pub sdk: BuiltinSdk,
    /// 版本发现主源 URL
    pub version_url: &'static str,
    /// 版本发现备源 URL（主源失败时回退）
    pub version_fallback_url: Option<&'static str>,
    /// 下载主源 URL 模板（包含占位符如 {version}/{os}/{arch}/{ext}）
    pub download_url: &'static str,
    /// 下载备源 URL 模板（下载主源失败时回退，可选）
    pub download_fallback_url: Option<&'static str>,
    /// 可选的 assets 详情 API URL 模板（仅 Java 使用）
    pub assets_url: Option<&'static str>,
}

pub const BUILTIN_SDK_CONFIG: &[SdkSourceConfig] = &[
    SdkSourceConfig {
        sdk: BuiltinSdk::Java,
        version_url: "https://api.adoptium.net/v3/info/available_releases",
        version_fallback_url: None,
        // {feature_version} = 大版本号，如 21 / 17 / 11
        // {os}              = linux / windows / mac (Adoptium 映射)
        // {arch}            = x64 / aarch64 (Adoptium 映射)
        download_url: "https://api.adoptium.net/v3/binary/latest/{feature_version}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
        download_fallback_url: None,
        assets_url: Some(
            "https://api.adoptium.net/v3/assets/latest/{feature_version}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse",
        ),
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Node,
        version_url: "https://nodejs.org/dist/index.json",
        version_fallback_url: None,
        // {version} = 完整版本号，如 v20.11.0（含 v 前缀）
        // {os}      = win / darwin / linux (Short 映射)
        // {arch}    = x64 / arm64 / x86 (Default 映射)
        // {ext}     = zip (win) / tar.gz (linux/mac)
        download_url: "https://nodejs.org/dist/{version}/node-{version}-{os}-{arch}.{ext}",
        download_fallback_url: None,
        assets_url: None,
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Python,
        // 主源：uv 维护的 Python 版本元数据，raw.githubusercontent.com 无速率限制
        version_url: "https://raw.githubusercontent.com/astral-sh/uv/main/crates/uv-python/download-metadata.json",
        // 备源：GitHub Releases API（完整但有限速/可用性问题）
        version_fallback_url: Some("https://api.github.com/repos/astral-sh/python-build-standalone/releases"),
        // {version}      = Python 版本号，如 3.12.0
        // {release_tag}  = 构建日期标签，如 20241216（动态发现）
        // {platform}     = 平台三元组，如 x86_64-pc-windows-msvc（自动检测）
        download_url: "https://github.com/astral-sh/python-build-standalone/releases/download/{release_tag}/cpython-{version}%2B{release_tag}-{platform}-install_only.tar.gz",
        download_fallback_url: None,
        assets_url: None,
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Maven,
        version_url: "", // Maven 暂无远程版本发现，仅精确版本
        version_fallback_url: None,
        // {version} = 完整版本号，如 3.9.9
        // {ext}     = zip (win) / tar.gz (linux/mac)
        download_url: "https://dlcdn.apache.org/maven/maven-3/{version}/binaries/apache-maven-{version}-bin.{ext}",
        download_fallback_url: None,
        assets_url: None,
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Go,
        // 官方版本列表 API：返回所有版本（含归档）的 JSON，含 sha256/size/files
        version_url: "https://go.dev/dl/?mode=json&include=all",
        version_fallback_url: None,
        // {version} = 版本号，如 1.26.5（不含 go 前缀，模板里 go{version} 组合）
        // {os}      = linux / darwin / windows (Default 映射)
        // {arch}    = amd64 / arm64 / 386 (Go 映射)
        // {ext}     = zip (win) / tar.gz (linux/mac)
        download_url: "https://go.dev/dl/go{version}.{os}-{arch}.{ext}",
        // 国内加速备源：Google 中国 CDN
        download_fallback_url: Some("https://golang.google.cn/dl/go{version}.{os}-{arch}.{ext}"),
        assets_url: None,
    },
];

/// 根据 BuiltinSdk 类型查找对应的 source config
pub fn find_builtin_sdk_config(sdk: &BuiltinSdk) -> Option<&'static SdkSourceConfig> {
    BUILTIN_SDK_CONFIG.iter().find(|c| c.sdk == *sdk)
}
