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

use crate::config::SdkConfig;
use crate::version::ResolvedVersion;

/// 按 SDK 构建下载 URL(模板渲染或使用直链)
///
/// `sdk_conf` 供 Custom 分支读取用户配置的 os_style/arch_style（标准 A 自定义 SDK 的
/// 风格配置化；未配置时保持 Default 风格的历史行为）
pub fn build_download_url(
    sdk: &Sdk,
    sdk_conf: &SdkConfig,
    template: &str,
    resolved: &ResolvedVersion,
) -> Result<String> {
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
        // Go：os 用 Default（linux/darwin/windows），arch 用 Go 风格（amd64/arm64/386）
        Sdk::Built(BuiltinSdk::Go) => TemplateRenderer::new()
            .var(PLACEHOLDER_OS, detect_os_with(OsStyle::Default))
            .var(PLACEHOLDER_ARCH, detect_arch_with(ArchStyle::Go))
            .var(PLACEHOLDER_OS_EXT, detect_ext())
            .var(PLACEHOLDER_VERSION, &resolved.full_version)
            .render(template),
        // custom SDK（含引擎型内置种子）：os/arch 风格可由 config 配置（如 helm/terraform 的
        // amd64 命名配 arch_style="go"），未配置时保持 Default 风格的历史行为
        Sdk::Custom(_) => {
            let os_style = sdk_conf
                .os_style
                .as_deref()
                .map(parse_os_style)
                .transpose()?
                .unwrap_or(OsStyle::Default);
            let arch_style = sdk_conf
                .arch_style
                .as_deref()
                .map(parse_arch_style)
                .transpose()?
                .unwrap_or(ArchStyle::Default);
            TemplateRenderer::new()
                .var(PLACEHOLDER_OS, detect_os_with(os_style))
                .var(PLACEHOLDER_ARCH, detect_arch_with(arch_style))
                .var(PLACEHOLDER_OS_EXT, detect_ext())
                .var(PLACEHOLDER_VERSION, &resolved.full_version)
                .render(template)
        }
    }
}

/// 解析用户配置的 os 风格字符串
fn parse_os_style(s: &str) -> Result<OsStyle> {
    match s {
        "default" => Ok(OsStyle::Default),
        "short" => Ok(OsStyle::Short),
        "adoptium" => Ok(OsStyle::Adoptium),
        _ => anyhow::bail!("Invalid os_style '{}'. Valid values: default, short, adoptium", s),
    }
}

/// 解析用户配置的 arch 风格字符串
fn parse_arch_style(s: &str) -> Result<ArchStyle> {
    match s {
        "default" => Ok(ArchStyle::Default),
        "adoptium" => Ok(ArchStyle::Adoptium),
        "python" => Ok(ArchStyle::Python),
        "go" => Ok(ArchStyle::Go),
        _ => anyhow::bail!("Invalid arch_style '{}'. Valid values: default, adoptium, python, go", s),
    }
}
