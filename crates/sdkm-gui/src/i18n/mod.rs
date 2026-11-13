//! GUI 文案：中文 / English 双语，按页面拆分子模块
//!
//! 结构：每个页面模块定义自己的文案 struct + ZH/EN 常量（同文件对照维护，
//! 加减字段编译器强制 zh/en 同步）；mod.rs 组合成 Lang 树，页面代码按
//! `t.overview.title` 层级访问。
//!
//! 边界约定：文案归 core、样式归前端——本表只覆盖
//! GUI 自身文案；core 事件/错误文案原样展示，常见错误经 [`errors`] 渐进映射。

// 文案表全量预定义（zh/en 同步维护），未实现页面暂不引用属预期
#[allow(dead_code)]
mod common;
#[allow(dead_code)]
mod diagnostics;
#[allow(dead_code)]
mod onboarding;
#[allow(dead_code)]
mod overview;
#[allow(dead_code)]
mod projects;
#[allow(dead_code)]
mod sdk;
#[allow(dead_code)]
mod settings;

pub mod errors;

pub use common::sdk_desc;

/// 全部界面文案（按页面分组）
pub struct Lang {
    pub common: common::CommonLang,
    pub onboarding: onboarding::OnboardingLang,
    pub overview: overview::OverviewLang,
    pub sdk: sdk::SdkLang,
    pub projects: projects::ProjectsLang,
    pub settings: settings::SettingsLang,
    pub diagnostics: diagnostics::DiagnosticsLang,
}

/// 简体中文
pub const ZH: Lang = Lang {
    common: common::ZH,
    onboarding: onboarding::ZH,
    overview: overview::ZH,
    sdk: sdk::ZH,
    projects: projects::ZH,
    settings: settings::ZH,
    diagnostics: diagnostics::ZH,
};

/// English
pub const EN: Lang = Lang {
    common: common::EN,
    onboarding: onboarding::EN,
    overview: overview::EN,
    sdk: sdk::EN,
    projects: projects::EN,
    settings: settings::EN,
    diagnostics: diagnostics::EN,
};

impl Lang {
    /// 按偏好取文案表
    pub fn pick(lang: crate::prefs::GuiLang) -> &'static Lang {
        match lang {
            crate::prefs::GuiLang::Zh => &ZH,
            crate::prefs::GuiLang::En => &EN,
        }
    }
}
