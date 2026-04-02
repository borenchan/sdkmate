use crate::sdk::BuiltinSdk;

/// sdk source config
pub struct SdkSourceConfig {
    pub sdk: BuiltinSdk,
    pub version_list_url : &'static str,
    pub download_url: &'static str,
    // pub get_download_fn: Fn(&self,version: &str) -> String,
}

pub const BUILTIN_SDK_CONFIG: &[SdkSourceConfig] =  &[
    SdkSourceConfig {
        sdk: BuiltinSdk::Java,
        version_list_url: "https://api.adoptium.net/v3/info/available_releases",
        // {feature_version} = 大版本号，如 21 / 17 / 11
        // {os}              = linux / windows / mac
        // {arch}            = x64 / aarch64
        download_url: "https://api.adoptium.net/v3/binary/latest/{feature_version}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Node,
        version_list_url: "https://nodejs.org/dist/index.json",
        //
        //  {version} = 完整版本号，如 v20.11.0（含 v 前缀）
        //  {os}      = win / darwin / linux
        //  {arch}    = x64 / arm64 / x86
        //  {ext}     = zip (win) / tar.gz (linux/mac)
        //
        download_url: "https://nodejs.org/dist/{version}/node-{version}-{os}-{arch}.{ext}",
    },
    SdkSourceConfig {
        sdk: BuiltinSdk::Python,
        version_list_url: "https://www.python.org/downloads/",
        download_url: "https://www.python.org/ftp/python/{version}/python-{version}-embed-{arch}.zip",
    },

];