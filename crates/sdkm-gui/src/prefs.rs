//! GUI 自身偏好（语言/主题/默认页面/密度），独立持久化于 `<sdkm_home>/gui.toml`
//!
//! 与 CLI 的 config.toml 完全分离——PRD 约定 GUI 偏好不得混入业务配置

use sdkm_util::path::get_sdkm_home;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

/// 界面语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuiLang {
    #[default]
    Zh,
    En,
}

/// 主题模式（跟随系统 / 浅色 / 深色）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuiThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// 界面密度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuiDensity {
    #[default]
    Standard,
    Compact,
}

/// 当前界面密度（渲染期读取；单 GUI 进程，主线程写入/读取）
static DENSITY: AtomicU8 = AtomicU8::new(0);

/// 设置当前密度（启动加载与设置页切换时调用）
pub fn set_density(d: GuiDensity) {
    DENSITY.store(
        match d {
            GuiDensity::Standard => 0,
            GuiDensity::Compact => 1,
        },
        Ordering::Relaxed,
    );
}

/// 当前密度
pub fn density() -> GuiDensity {
    if DENSITY.load(Ordering::Relaxed) == 1 {
        GuiDensity::Compact
    } else {
        GuiDensity::Standard
    }
}

/// 卡片内边距
pub fn card_pad() -> f32 {
    density().card_pad()
}

impl GuiDensity {
    /// 卡片内边距
    pub fn card_pad(self) -> f32 {
        match self {
            GuiDensity::Standard => 16.,
            GuiDensity::Compact => 12.,
        }
    }

    /// 页面左右内边距
    pub fn page_pad_x(self) -> f32 {
        match self {
            GuiDensity::Standard => 24.,
            GuiDensity::Compact => 16.,
        }
    }

    /// 页面上下内边距
    pub fn page_pad_y(self) -> f32 {
        match self {
            GuiDensity::Standard => 20.,
            GuiDensity::Compact => 12.,
        }
    }

    /// 列表行上下内边距
    pub fn row_pad_y(self) -> f32 {
        match self {
            GuiDensity::Standard => 6.,
            GuiDensity::Compact => 4.,
        }
    }

    /// 侧边导航项上下内边距
    pub fn nav_pad_y(self) -> f32 {
        match self {
            GuiDensity::Standard => 8.,
            GuiDensity::Compact => 6.,
        }
    }

    /// 侧边栏宽度
    pub fn sidebar_width(self) -> f32 {
        match self {
            GuiDensity::Standard => 176.,
            GuiDensity::Compact => 164.,
        }
    }

    /// 统计卡内边距
    pub fn stat_pad(self) -> f32 {
        match self {
            GuiDensity::Standard => 12.,
            GuiDensity::Compact => 10.,
        }
    }

    /// 顶部 logo 区下边距
    pub fn logo_pad_bottom(self) -> f32 {
        match self {
            GuiDensity::Standard => 24.,
            GuiDensity::Compact => 16.,
        }
    }
}

/// GUI 偏好（gui.toml 顶层结构）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuiPrefs {
    pub lang: GuiLang,
    pub theme_mode: GuiThemeMode,
    /// 启动时打开的页面 id（"overview" / "sdk" / "projects" / "settings" / "diagnostics"）
    pub default_page: String,
    pub density: GuiDensity,
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            lang: GuiLang::Zh,
            theme_mode: GuiThemeMode::System,
            default_page: "overview".into(),
            density: GuiDensity::Standard,
        }
    }
}

/// gui.toml 路径：`<sdkm_home>/gui.toml`（SDKM_HOME 注入友好，测试可沙箱）
pub fn gui_prefs_path() -> anyhow::Result<PathBuf> {
    Ok(get_sdkm_home()?.join("gui.toml"))
}

impl GuiPrefs {
    /// 从磁盘加载；文件不存在或解析失败一律回退默认值（GUI 必须总能启动）
    pub fn load() -> Self {
        let Ok(path) = gui_prefs_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// 原子保存（写入-重命名，失败静默——偏好丢失不致命）
    pub fn save(&self) {
        let Ok(path) = gui_prefs_path() else {
            return;
        };
        let Ok(body) = toml::to_string_pretty(self) else {
            return;
        };
        let tmp = path.with_extension("toml.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}
