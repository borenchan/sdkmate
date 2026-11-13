//! 通用 + 导航文案

/// 通用文案
pub struct CommonLang {
    pub app_subtitle: &'static str,
    pub nav_overview: &'static str,
    pub nav_sdk: &'static str,
    pub nav_projects: &'static str,
    pub nav_settings: &'static str,
    pub nav_diagnostics: &'static str,
    pub running: &'static str,
    pub lang_name: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub theme_system: &'static str,
    pub btn_cancel: &'static str,
    pub btn_ok: &'static str,
    pub btn_confirm: &'static str,
    pub btn_save: &'static str,
    pub btn_refresh: &'static str,
    pub btn_prev: &'static str,
    pub btn_next: &'static str,
    pub loading: &'static str,
    pub empty_list: &'static str,
    pub no_results: &'static str,
    pub op_running: &'static str,
    pub op_done: &'static str,
    pub op_failed: &'static str,
    pub err_title: &'static str,
    pub retry: &'static str,
}

/// 简体中文
pub const ZH: CommonLang = CommonLang {
    app_subtitle: "SDK 版本管理器",
    nav_overview: "概览",
    nav_sdk: "SDK管理",
    nav_projects: "项目管理",
    nav_settings: "设置",
    nav_diagnostics: "诊断与维护",
    running: "运行正常",
    lang_name: "中文",
    theme_light: "浅色",
    theme_dark: "深色",
    theme_system: "跟随系统",
    btn_cancel: "取消",
    btn_ok: "确定",
    btn_confirm: "确认",
    btn_save: "保存",
    btn_refresh: "刷新",
    btn_prev: "上一页",
    btn_next: "下一页",
    loading: "加载中...",
    empty_list: "暂无数据",
    no_results: "无匹配结果",
    op_running: "正在执行",
    op_done: "完成",
    op_failed: "失败",
    err_title: "出错了",
    retry: "重试",
};

/// English
pub const EN: CommonLang = CommonLang {
    app_subtitle: "SDK Manager",
    nav_overview: "Overview",
    nav_sdk: "SDK Manager",
    nav_projects: "Projects",
    nav_settings: "Settings",
    nav_diagnostics: "Diagnostics",
    running: "Running",
    lang_name: "English",
    theme_light: "Light",
    theme_dark: "Dark",
    theme_system: "System",
    btn_cancel: "Cancel",
    btn_ok: "OK",
    btn_confirm: "Confirm",
    btn_save: "Save",
    btn_refresh: "Refresh",
    btn_prev: "Prev",
    btn_next: "Next",
    loading: "Loading...",
    empty_list: "No data",
    no_results: "No results",
    op_running: "Running",
    op_done: "Done",
    op_failed: "Failed",
    err_title: "Error",
    retry: "Retry",
};

/// SDK 类型描述（GUI 文案，非统计数据）
///
/// 已知 SDK 中英双语；未登记的内置 SDK 统一「内置」，自定义 SDK 统一「用户自定义 SDK」。
pub fn sdk_desc(name: &str, lang: crate::prefs::GuiLang) -> &'static str {
    use sdkm_util::builtin::find_seed;
    let zh = matches!(lang, crate::prefs::GuiLang::Zh);
    match name {
        "java" => {
            if zh {
                "Java 开发工具包"
            } else {
                "Java Development Kit"
            }
        }
        "maven" => {
            if zh {
                "构建工具"
            } else {
                "Build Tool"
            }
        }
        "node" => {
            if zh {
                "JavaScript 运行时"
            } else {
                "JavaScript Runtime"
            }
        }
        "python" => {
            if zh {
                "Python 解释器"
            } else {
                "Python Interpreter"
            }
        }
        "go" => {
            if zh {
                "Go 语言工具链"
            } else {
                "Go Toolchain"
            }
        }
        "docker" => {
            if zh {
                "容器化工具"
            } else {
                "Container Tool"
            }
        }
        "git" => {
            if zh {
                "版本控制工具"
            } else {
                "Version Control"
            }
        }
        _ if find_seed(name).is_some() => {
            if zh {
                "内置"
            } else {
                "Built-in"
            }
        }
        _ => {
            if zh {
                "用户自定义 SDK"
            } else {
                "User-defined SDK"
            }
        }
    }
}
