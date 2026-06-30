// 下载 URL 构建(install 专属,不属于"版本解析")
//
// 各 SDK 的 os/arch 风格原本分散在 install/resolver.rs 的各 strategy 结构体里,
// 此处集中为一个按 SDK 分发的自由函数。

use anyhow::Result;
use util::config_helper::{
    ArchStyle, OsStyle, PLACEHOLDER_ARCH, PLACEHOLDER_FEATURE_VERSION, PLACEHOLDER_OS, PLACEHOLDER_OS_EXT,
    PLACEHOLDER_PLATFORM, PLACEHOLDER_RELEASE_TAG, PLACEHOLDER_VERSION, TemplateRenderer, detect_arch_with, detect_ext,
    detect_os_with, detect_platform_triple,
};
use util::sdk::{BuiltinSdk, Sdk};

use crate::version::ResolvedVersion;

/// 按 SDK 构建下载 URL(模板渲染或使用直链)
pub fn build_download_url(sdk: &Sdk, template: &str, resolved: &ResolvedVersion) -> Result<String> {
    match sdk {
        Sdk::Built(BuiltinSdk::Java) => {
            let mut r = TemplateRenderer::new()
                .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Adoptium))
                .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Adoptium))
                .var(PLACEHOLDER_OS_EXT, detect_ext());
            if let Some(fv) = &resolved.feature_version {
                r = r.var(PLACEHOLDER_FEATURE_VERSION, fv);
            }
            r.render(template)
        }
        Sdk::Built(BuiltinSdk::Node) => TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Short))
            .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Default))
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, format!("v{}", resolved.full_version))
            .render(template),
        Sdk::Built(BuiltinSdk::Python) => {
            // 如果已有直链(来自 uv metadata),直接使用
            if let Some(url) = &resolved.download_url {
                return Ok(url.clone());
            }
            // 否则使用模板渲染
            let mut r = TemplateRenderer::new()
                .var(PLACEHOLDER_VERSION, &resolved.full_version)
                .var(PLACEHOLDER_PLATFORM, detect_platform_triple()?);
            if let Some(tag) = &resolved.release_tag {
                r = r.var(PLACEHOLDER_RELEASE_TAG, tag);
            }
            r.render(template)
        }
        Sdk::Built(BuiltinSdk::Maven) => TemplateRenderer::new()
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .render(template),
        // custom SDK 沿用原 ConfigBasedStrategy::default() 的 Default 风格
        Sdk::Custom(_) => TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Default))
            .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Default))
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .render(template),
    }
}
