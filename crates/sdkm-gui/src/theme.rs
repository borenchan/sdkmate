//! 主题：把设计图配色覆盖到 gpui-kit Theme（浅色为设计基准，深色按同色系推导）
//!
//! 设计图是浅色蓝调：内容区浅蓝灰底 + 纯白卡片 + 主蓝 #3B82F6 + 绿成功/红危险。
//! 覆盖在 gpui-kit 默认主题之上（`Theme::update`），语义字段各组件自动跟随。
//!
//! 语义 soft 色（标签/图标底色等 gpui-kit 主题未覆盖的部分）经 [`GuiTokens`]
//! Global 与主题联动：`apply` 时按生效模式 set_global，组件统一
//! `cx.global::<GuiTokens>()` 取色，禁止散落硬编码。

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, rgb};

/// GUI 语义色 token（与主题模式联动，apply 时刷新）
#[derive(Clone, Copy, Debug)]
pub struct GuiTokens {
    /// 成功 soft（"已激活/正常" pill、状态条）
    pub success_bg: Hsla,
    pub success_fg: Hsla,
    /// 主蓝 soft（图标浅底、蓝调提示条）
    pub primary_soft_bg: Hsla,
    pub primary_soft_fg: Hsla,
    /// 危险 soft（错误条、卸载确认条）
    pub danger_bg: Hsla,
    pub danger_fg: Hsla,
    /// 统计卡语义图标色（蓝=数据 / 绿=健康，与设计图一致）
    pub icon_blue: Hsla,
    pub icon_green: Hsla,
}

impl Global for GuiTokens {}

/// 取当前语义 token（theme::apply 未调用前的兜底为浅色值）
pub fn tokens(cx: &App) -> GuiTokens {
    cx.try_global::<GuiTokens>().copied().unwrap_or_else(light_tokens)
}

fn light_tokens() -> GuiTokens {
    GuiTokens {
        success_bg: rgb(0xecfdf5).into(),
        success_fg: rgb(0x059669).into(),
        primary_soft_bg: rgb(0xeef4fd).into(),
        primary_soft_fg: rgb(0x2b7fff).into(),
        danger_bg: rgb(0xfef2f2).into(),
        danger_fg: rgb(0xef4444).into(),
        icon_blue: rgb(0x2b7fff).into(),
        icon_green: rgb(0x059669).into(),
    }
}

fn dark_tokens() -> GuiTokens {
    GuiTokens {
        success_bg: rgb(0x10b981).opacity(0.16).into(),
        success_fg: rgb(0x34d399).into(),
        primary_soft_bg: rgb(0x3b82f6).opacity(0.16).into(),
        primary_soft_fg: rgb(0x60a5fa).into(),
        danger_bg: rgb(0xef4444).opacity(0.16).into(),
        danger_fg: rgb(0xf87171).into(),
        icon_blue: rgb(0x60a5fa).into(),
        icon_green: rgb(0x34d399).into(),
    }
}

/// 浅色：设计图取色
fn light_overrides(t: &mut Theme) {
    // 背景层级
    t.background = rgb(0xf4f8fc).into(); // 内容区浅蓝灰
    t.popover = rgb(0xffffff).into(); // 卡片纯白（GUI 卡片容器统一用 popover）
    t.sidebar = rgb(0xfbfdff).into(); // 侧边栏淡白（与内容区/卡片都区分）
    t.foreground = rgb(0x0f172a).into();
    t.muted = rgb(0xf1f5f9).into();
    t.muted_foreground = rgb(0x64748b).into();
    t.border = rgb(0xe2e8f0).into();

    // 主蓝（链接/选中/语义）
    t.primary = rgb(0x3b82f6).into();
    t.primary_hover = rgb(0x2f6fe0).into();
    t.primary_active = rgb(0x2861c7).into();
    t.primary_foreground = rgb(0xffffff).into();
    t.ring = rgb(0x93c5fd).into();
    t.link = rgb(0x2b7fff).into();

    // 按钮：主操作蓝底白字（gpui-kit 按钮读 button_* 而非 primary——设计图蓝按钮的关键）
    t.button_primary = rgb(0x3b82f6).into();
    t.button_primary_hover = rgb(0x2f6fe0).into();
    t.button_primary_active = rgb(0x2861c7).into();
    t.button_primary_foreground = rgb(0xffffff).into();
    // 危险按钮红
    t.button_danger = rgb(0xef4444).into();
    t.button_danger_hover = rgb(0xdc2626).into();
    t.button_danger_active = rgb(0xb91c1c).into();
    t.button_danger_foreground = rgb(0xffffff).into();
    // 次级按钮（切换等描边灰）——与设计图的浅灰描边一致
    t.button_secondary = rgb(0xffffff).into();
    t.button_secondary_hover = rgb(0xf1f5f9).into();
    t.button_secondary_active = rgb(0xe2e8f0).into();
    t.button_secondary_foreground = rgb(0x334155).into();

    // 侧边导航（淡白底 + 浅蓝选中）
    t.sidebar_accent = rgb(0xeaf2ff).into();
    t.sidebar_accent_foreground = rgb(0x2b7fff).into();
    t.sidebar_border = rgb(0xeef2f7).into();
    t.sidebar_primary = rgb(0x3b82f6).into();
    t.sidebar_primary_foreground = rgb(0xffffff).into();

    // 语义色
    t.success = rgb(0x10b981).into();
    t.danger = rgb(0xef4444).into();
    t.warning = rgb(0xf59e0b).into();
    t.info = rgb(0x3b82f6).into();

    // 版本 tabs：选中蓝字蓝下划线（与主蓝一致）
    t.tab_active = rgb(0xffffff).into();
    t.tab_active_foreground = rgb(0x2b7fff).into();
    t.tab_bar = rgb(0xf4f8fc).into();
    t.tab_foreground = rgb(0x64748b).into();

    // 表格 / 标题栏
    t.table_head = rgb(0xf8fafc).into();
    t.table_head_foreground = rgb(0x475569).into();
    t.title_bar = rgb(0xf4f8fc).into();
    t.title_bar_border = rgb(0xe5eaf1).into();
    // 输入框边框（白底上需清晰可见；聚焦时为蓝 ring）
    t.input = rgb(0xcbd5e1).into();

    // 圆角（设计图卡片/按钮统一中等圆角）
    t.radius = gpui_kit::px(8.);
}

/// 深色：同色系推导（深蓝黑底，主蓝提亮）
fn dark_overrides(t: &mut Theme) {
    t.background = rgb(0x0b1220).into();
    t.popover = rgb(0x111a2c).into();
    t.foreground = rgb(0xe2e8f0).into();
    t.muted = rgb(0x1b2537).into();
    t.muted_foreground = rgb(0x94a3b8).into();
    t.border = rgb(0x1e293b).into();

    t.primary = rgb(0x3b82f6).into();
    t.primary_hover = rgb(0x4a8dff).into();
    t.primary_active = rgb(0x2861c7).into();
    t.ring = rgb(0x3b82f6).into();
    t.link = rgb(0x60a5fa).into();

    t.button_primary = rgb(0x3b82f6).into();
    t.button_primary_hover = rgb(0x4a8dff).into();
    t.button_primary_active = rgb(0x2f6fe0).into();
    t.button_primary_foreground = rgb(0xffffff).into();
    t.button_danger = rgb(0xef4444).into();
    t.button_danger_hover = rgb(0xf87171).into();
    t.button_danger_active = rgb(0xdc2626).into();
    t.button_danger_foreground = rgb(0xffffff).into();
    t.button_secondary = rgb(0x1b2537).into();
    t.button_secondary_hover = rgb(0x243149).into();
    t.button_secondary_active = rgb(0x2a3a58).into();
    t.button_secondary_foreground = rgb(0xcbd5e1).into();

    t.sidebar = rgb(0x0e1626).into();
    t.sidebar_accent = rgb(0x16283f).into();
    t.sidebar_accent_foreground = rgb(0x60a5fa).into();
    t.sidebar_border = rgb(0x182234).into();

    t.success = rgb(0x10b981).into();
    t.danger = rgb(0xef4444).into();

    t.tab_active = rgb(0x111a2c).into();
    t.tab_active_foreground = rgb(0x60a5fa).into();
    t.tab_bar = rgb(0x0e1626).into();
    t.tab_foreground = rgb(0x94a3b8).into();

    t.table_head = rgb(0x131e30).into();
    t.table_head_foreground = rgb(0x94a3b8).into();
    t.title_bar = rgb(0x0b1220).into();
    t.title_bar_border = rgb(0x182234).into();
    // 输入框边框（深底上可见）
    t.input = rgb(0x2a3a55).into();
}

/// 存储占用环形图分类色（Java/Node.js/Python/其他，浅深两套）
pub fn chart_colors(dark: bool) -> [Hsla; 4] {
    if dark {
        [
            rgb(0x60a5fa).into(),
            rgb(0x34d399).into(),
            rgb(0xfbbf24).into(),
            rgb(0x64748b).into(),
        ]
    } else {
        [
            rgb(0x3b82f6).into(),
            rgb(0x10b981).into(),
            rgb(0xf59e0b).into(),
            rgb(0x94a3b8).into(),
        ]
    }
}

/// 应用主题：System 跟随系统外观，再覆盖设计色 + 刷新语义 token
pub fn apply(mode: crate::prefs::GuiThemeMode, cx: &mut App) {
    match mode {
        crate::prefs::GuiThemeMode::System => {
            Theme::sync_system_appearance(None, cx);
        }
        crate::prefs::GuiThemeMode::Light => {
            Theme::change(ThemeMode::Light, None, cx);
        }
        crate::prefs::GuiThemeMode::Dark => {
            Theme::change(ThemeMode::Dark, None, cx);
        }
    }
    // sync/change 后按最终生效模式覆盖设计色 + 同步 token
    let dark = Theme::global(cx).mode == ThemeMode::Dark;
    cx.set_global(if dark { dark_tokens() } else { light_tokens() });
    Theme::update(cx, |t| {
        if dark {
            dark_overrides(t);
        } else {
            light_overrides(t);
        }
    });
}

/// 当前是否深色模式（图表配色等场景用）
pub fn is_dark(cx: &App) -> bool {
    Theme::global(cx).mode == ThemeMode::Dark
}
