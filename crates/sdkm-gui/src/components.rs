//! 复用 UI 组件：卡片容器、统计卡、SDK 图标、键值行、快速操作块等（各页面共用）
//!
//! 组件化原则（用户要求）：能复用的页面元素一律抽到这里，页面间零重复实现。

use crate::pages::sdk::VersionTab;
use crate::prefs::density;
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 白色卡片容器（设计图基础容器：纯白底 + 圆角 + 微边框）
/// 返回 Stateful<Div>，调用方可继续链样式/子元素
pub fn card(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id.into())
        .bg(theme.popover)
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
}

/// 卡片标题行（左侧标题 + 右侧可选附加元素）
pub fn card_header(title: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex().items_center().justify_between().pb_2().child(
        div()
            .text_size(px(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.foreground)
            .child(title.into()),
    )
}

/// 统计卡片（概览页顶部 4 卡：图标 + 标签 + 大数字 + 副文本；图标 soft 色由调用方按 token 传入）
pub fn stat_card(
    id: impl Into<ElementId>,
    icon: IconName,
    icon_fg: Hsla,
    icon_bg: Hsla,
    label: &str,
    value: &str,
    sub: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    card(id, cx)
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p(px(density().stat_pad()))
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .min_w_0()
                .child(
                    // 图标：语义 soft 底（浅/深两套随主题，由调用方按 token 传入）
                    h_flex()
                        .size(px(30.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .justify_center()
                        .bg(icon_bg)
                        .child(Icon::new(icon).with_size(px(20.)).text_color(icon_fg)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(label.to_string()),
                ),
        )
        .child(
            div()
                .text_size(px(26.))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.foreground)
                .truncate()
                .child(value.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .truncate()
                .child(sub.to_string()),
        )
}

/// SDK 视觉映射：（Lucide 图标，底色，前景色）——品牌色浅深两套，结构留后续换真 logo SVG
pub fn sdk_visual(name: &str, dark: bool) -> (IconName, Hsla, Hsla) {
    match (name, dark) {
        // 浅色：品牌浅底 + 品牌深色
        ("java", false) => (IconName::Coffee, rgb(0xfef3e2).into(), rgb(0xe76f00).into()),
        ("node", false) => (IconName::Hexagon, rgb(0xeaf7ee).into(), rgb(0x3c873a).into()),
        ("python", false) => (IconName::FileCode, rgb(0xeaf2fd).into(), rgb(0x3776ab).into()),
        ("go", false) => (IconName::Zap, rgb(0xe2f4f7).into(), rgb(0x00acd7).into()),
        ("maven", false) => (IconName::Feather, rgb(0xfdeaea).into(), rgb(0xc74634).into()),
        ("docker", false) => (IconName::Container, rgb(0xe5f0fd).into(), rgb(0x2496ed).into()),
        ("git", false) => (IconName::GitBranch, rgb(0xfdeee4).into(), rgb(0xf05033).into()),
        (_, false) => (IconName::Puzzle, rgb(0xeef2f7).into(), rgb(0x64748b).into()),
        // 深色：品牌色低透明底 + 提亮前景
        ("java", true) => (IconName::Coffee, rgb(0xe76f00).opacity(0.16).into(), rgb(0xff9a4d).into()),
        ("node", true) => (IconName::Hexagon, rgb(0x3c873a).opacity(0.16).into(), rgb(0x6ee07a).into()),
        ("python", true) => (IconName::FileCode, rgb(0x3776ab).opacity(0.16).into(), rgb(0x6bb3e8).into()),
        ("go", true) => (IconName::Zap, rgb(0x00acd7).opacity(0.16).into(), rgb(0x5cd6f5).into()),
        ("maven", true) => (IconName::Feather, rgb(0xc74634).opacity(0.16).into(), rgb(0xf08777).into()),
        ("docker", true) => (IconName::Container, rgb(0x2496ed).opacity(0.16).into(), rgb(0x6cb8f5).into()),
        ("git", true) => (IconName::GitBranch, rgb(0xf05033).opacity(0.16).into(), rgb(0xff8a70).into()),
        (_, true) => (IconName::Puzzle, rgb(0x64748b).opacity(0.16).into(), rgb(0x94a3b8).into()),
    }
}

/// SDK 图标（圆角彩色底 + 线性图标），列表/详情/表格统一走这里
pub fn sdk_icon(name: &str, size: f32, cx: &App) -> impl IntoElement {
    let (icon, bg, fg) = sdk_visual(name, crate::theme::is_dark(cx));
    h_flex()
        .size(px(size))
        .rounded(px(size / 4.))
        .flex_shrink_0()
        .justify_center()
        .bg(bg)
        .child(Icon::new(icon).with_size(px(size * 0.55)).text_color(fg))
}

/// 状态 pill（半圆角标签：底色+前景色随主题 token，"已激活/正常"等复用）
pub fn pill(text: &str, bg: Hsla, fg: Hsla) -> impl IntoElement {
    div()
        .px_2p5()
        .py_1()
        .rounded_full()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .bg(bg)
        .text_color(fg)
        .child(text.to_string())
}

/// 状态 pill 带前置圆点（设计图详情头部"● 已安装"样式）
pub fn pill_dot(text: &str, dot_color: Hsla, bg: Hsla, fg: Hsla) -> impl IntoElement {
    h_flex()
        .px_2p5()
        .py_1()
        .gap_1p5()
        .rounded_full()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .bg(bg)
        .text_color(fg)
        .child(div().size_1p5().rounded_full().bg(dot_color))
        .child(text.to_string())
}

/// 键值行（系统信息/诊断卡：label 灰 + value 右对齐）
pub fn kv_row(label: &str, value: &str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .items_center()
        .justify_between()
        .gap_3()
        .min_w_0()
        .py_1()
        .child(
            div()
                .text_sm()
                .flex_shrink_0()
                .text_color(theme.muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(theme.foreground)
                .truncate()
                .child(value.to_string()),
        )
}

/// 快速操作块（概览页 2×2 / 项目页底部按钮：图标 + 标题 + 描述，可点击）
pub fn action_tile(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &str,
    desc: &str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .gap_1()
        .p_3()
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .hover(|s| s.bg(theme.sidebar_accent).border_color(theme.primary))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    h_flex()
                        .size(px(30.))
                        .rounded(px(8.))
                        .justify_center()
                        .bg(theme.sidebar_accent)
                        .child(Icon::new(icon).text_color(theme.sidebar_accent_foreground)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(label.to_string()),
                ),
        )
        .child(
            div()
                .min_w_0()
                .text_xs()
                .text_color(theme.muted_foreground)
                .truncate()
                .child(desc.to_string()),
        )
}

/// 列表分页条（上一页 / 页码 / 下一页）
pub fn pager(
    base_id: &'static str,
    page: usize,
    total_pages: usize,
    prev_label: &'static str,
    next_label: &'static str,
    cx: &Context<crate::app::GuiApp>,
    on_prev: impl Fn(&mut crate::app::GuiApp, &mut Context<crate::app::GuiApp>) + 'static,
    on_next: impl Fn(&mut crate::app::GuiApp, &mut Context<crate::app::GuiApp>) + 'static,
) -> impl IntoElement {
    let theme = cx.theme();
    let on_prev = std::rc::Rc::new(on_prev);
    let on_next = std::rc::Rc::new(on_next);
    let nav_btn =
        |id: &'static str,
         label: &'static str,
         icon: IconName,
         enabled: bool,
         cx: &Context<crate::app::GuiApp>,
         on: std::rc::Rc<dyn Fn(&mut crate::app::GuiApp, &mut Context<crate::app::GuiApp>)>| {
            div()
                .id(ElementId::Name(format!("{base_id}-{id}").into()))
                .flex()
                .items_center()
                .gap_1()
                .px_2p5()
                .py_1()
                .rounded(px(6.))
                .text_xs()
                .text_color(if enabled {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .when(enabled, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(theme.muted))
                        .on_click(cx.listener(move |app, _, _, cx| on(app, cx)))
                })
                .child(Icon::new(icon).small())
                .child(label.to_string())
        };

    h_flex()
        .gap_1()
        .items_center()
        .justify_end()
        .w_full()
        .child(nav_btn(
            "prev",
            prev_label,
            IconName::ChevronLeft,
            page > 0,
            cx,
            on_prev.clone(),
        ))
        .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
            "{} / {}",
            page + 1,
            total_pages.max(1)
        )))
        .child(nav_btn(
            "next",
            next_label,
            IconName::ChevronRight,
            page + 1 < total_pages,
            cx,
            on_next.clone(),
        ))
}

/// 危险操作块（结构同 action_tile，图标/文字/边框红色）
pub fn action_tile_danger(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &str,
    desc: &str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .gap_1()
        .p_3()
        .rounded(px(10.))
        .border_1()
        .border_color(tks.danger_bg)
        .bg(theme.popover)
        .hover(|s| s.bg(tks.danger_bg).border_color(theme.danger))
        .cursor_pointer()
        .on_click(on_click)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    h_flex()
                        .size(px(30.))
                        .rounded(px(8.))
                        .justify_center()
                        .bg(tks.danger_bg)
                        .child(Icon::new(icon).text_color(theme.danger)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.danger)
                        .child(label.to_string()),
                ),
        )
        .child(div().text_xs().text_color(theme.muted_foreground).child(desc.to_string()))
}

/// 页面级空状态（图标 + 标题 + 描述，垂直居中）
pub fn empty_state(icon: IconName, title: &str, desc: &str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(Icon::new(icon).with_size(px(48.)).text_color(theme.muted_foreground))
        .child(
            div()
                .text_size(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(title.to_string()),
        )
        .child(div().text_sm().text_color(theme.muted_foreground).child(desc.to_string()))
}

/// 格式化字节数为人类可读（1.24 GB / 523 MB / 12 KB）
pub fn fmt_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

// ── SDK 表格（概览页 / SDK 管理页共用）──

/// SDK 表格行点击行为（页内导航差异由页面决定，表格组件零页面知识）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkTableAction {
    /// 概览页：跳转 SDK 管理页并选中该行
    JumpAndSelect,
    /// SDK 管理页：选中该行
    Select,
}

/// SDK 表格（带表头：名称/当前版本/已安装版本/状态/操作）
///
/// `rows` 为（全量索引, 行）对——过滤由调用方完成并保留原索引，
/// 保证选中态与管理按钮指向全量列表的同一行。
pub fn sdk_table(
    app: &crate::app::GuiApp,
    rows: &[(usize, &crate::data::SdkListItem)],
    action: SdkTableAction,
    cx: &Context<crate::app::GuiApp>,
) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();

    // ── 表头 ──
    let header = h_flex()
        .px_3()
        .pb(px(density().row_pad_y() + 2.))
        .gap_4()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(
            div()
                .flex_grow(2.)
                .flex_basis(px(0.))
                .min_w(px(180.))
                .child(t.overview.col_name.to_string()),
        )
        .child(
            div()
                .flex_grow(1.)
                .flex_basis(px(0.))
                .min_w(px(110.))
                .child(t.overview.col_current.to_string()),
        )
        .child(
            div()
                .flex_grow(1.)
                .flex_basis(px(0.))
                .min_w(px(90.))
                .child(t.overview.col_installed.to_string()),
        )
        .child(
            div()
                .flex_grow(1.)
                .flex_basis(px(0.))
                .min_w(px(110.))
                .child(t.overview.col_status.to_string()),
        );

    // ── 行 ──
    let body = rows.iter().fold(v_flex(), |acc, (idx, row)| {
        let idx = *idx;
        let active = row.current.is_some();
        acc.child(
            h_flex()
                .id(ElementId::NamedInteger("sdk-row".into(), idx as u64))
                .px_3()
                .py(px(density().row_pad_y()))
                .gap_4()
                .items_center()
                .rounded(px(8.))
                .border_b_1()
                .border_color(theme.border)
                .hover(|s| s.bg(theme.sidebar_accent))
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| match action {
                    SdkTableAction::JumpAndSelect => {
                        app.sdk_page.selected = Some(idx);
                        app.navigate(crate::app::Page::Sdk, cx);
                    }
                    SdkTableAction::Select => {
                        app.sdk_page.selected = Some(idx);
                        app.sdk_page.version_tab = VersionTab::Local;
                        app.sdk_page.remote = None;
                        cx.notify();
                    }
                }))
                .child(
                    // 名称列：图标 + 名称 + 类型描述
                    h_flex()
                        .flex_grow(2.)
                        .flex_basis(px(0.))
                        .min_w(px(180.))
                        .min_w_0()
                        .gap_3()
                        .items_center()
                        .child(sdk_icon(&row.name, 36., cx))
                        .child(
                            v_flex()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .truncate()
                                        .child(row.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(crate::i18n::sdk_desc(&row.name, app.prefs.lang)),
                                ),
                        ),
                )
                .child(
                    // 当前版本列（激活状态由右侧状态列体现）
                    h_flex()
                        .flex_grow(1.)
                        .flex_basis(px(0.))
                        .min_w(px(110.))
                        .min_w_0()
                        .overflow_hidden()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .truncate()
                                .child(row.current.clone().unwrap_or_else(|| "—".into())),
                        ),
                )
                .child(
                    // 已安装版本数量列（固定行高 + 垂直居中，和图标列一致）
                    h_flex()
                        .flex_grow(1.)
                        .flex_basis(px(0.))
                        .min_w(px(90.))
                        .h(px(36.))
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(row.versions.len().to_string()),
                        ),
                )
                .child(
                    // 状态列
                    h_flex()
                        .flex_grow(1.)
                        .flex_basis(px(0.))
                        .min_w(px(110.))
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .child(div().size_2().rounded_full().bg(if row.installed {
                            theme.success
                        } else {
                            theme.muted_foreground
                        }))
                        .child(div().text_sm().text_color(theme.muted_foreground).truncate().child(if active {
                            t.overview.tag_active.to_string()
                        } else if row.installed {
                            t.overview.status_installed.to_string()
                        } else {
                            t.overview.status_not_installed.to_string()
                        })),
                ),
        )
    });

    v_flex().gap_1().child(header).child(body)
}

/// 筛选 chip（设计图样式：选中蓝底白字，未选中卡底灰字；token 配色随主题）
pub fn filter_chip(
    id: impl Into<ElementId>,
    label: String,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id(id.into())
        .px_3()
        .py_1p5()
        .rounded(px(8.))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(selected, |el| el.bg(theme.primary).text_color(theme.primary_foreground))
        .when(!selected, |el| {
            el.bg(theme.popover)
                .text_color(theme.muted_foreground)
                .border_1()
                .border_color(theme.border)
                .hover(|s| s.text_color(theme.foreground))
        })
        .on_click(on_click)
        .child(label)
}

/// 版本管理 segmented tabs（设计图 p2：胶囊按钮组，选中蓝底白字）
///
/// `tabs` 为 (元素 id, 标签) 对；点击经 `on_select(索引)` 回调到页面。
pub fn segmented_tabs(
    base_id: &'static str,
    tabs: &[(&'static str, &'static str)],
    selected_index: usize,
    cx: &Context<crate::app::GuiApp>,
    on_select: impl Fn(&mut crate::app::GuiApp, usize, &mut Context<crate::app::GuiApp>) + 'static,
) -> impl IntoElement {
    let theme = cx.theme();
    let on_select = std::rc::Rc::new(on_select);
    let inner = tabs.iter().enumerate().fold(h_flex().gap_1(), |acc, (i, (el_id, label))| {
        let selected = i == selected_index;
        acc.child(
            div()
                .id(ElementId::Name(format!("{base_id}-{el_id}").into()))
                .px_3()
                .py_1p5()
                .rounded(px(8.))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(selected, |el| el.bg(theme.primary).text_color(theme.primary_foreground))
                .when(!selected, |el| {
                    el.text_color(theme.muted_foreground)
                        .hover(|s| s.bg(theme.popover).text_color(theme.foreground))
                })
                .on_click({
                    let on_select = on_select.clone();
                    cx.listener(move |app, _, _, cx| {
                        on_select(app, i, cx);
                    })
                })
                .child((*label).to_string()),
        )
    });
    div()
        .id(ElementId::Name(base_id.into()))
        .rounded(px(10.))
        .bg(theme.muted)
        .p_1()
        .child(inner)
}

// ── 项目自绘按钮（替代三方 Button：手型光标/hover/禁用态全部自主可控）──

/// 按钮变体（配色走主题 token，浅深自动跟随）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtnVariant {
    /// 主操作：蓝底白字
    Primary,
    /// 危险操作：红底白字
    Danger,
    /// 次要操作：卡底描边
    Outline,
    /// 弱操作：无边框透明底
    Ghost,
}

/// 按钮尺寸
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtnSize {
    Small,
    Medium,
    Large,
}

/// 自绘按钮：手型光标 + hover 三态 + 禁用/加载态
#[allow(clippy::too_many_arguments)]
pub fn btn(
    id: impl Into<ElementId>,
    variant: BtnVariant,
    size: BtnSize,
    label: &str,
    icon: Option<IconName>,
    disabled: bool,
    loading: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let (bg, fg, hover_bg) = match variant {
        BtnVariant::Primary => (
            theme.button_primary,
            theme.button_primary_foreground,
            theme.button_primary_hover,
        ),
        BtnVariant::Danger => (theme.button_danger, theme.button_danger_foreground, theme.button_danger_hover),
        BtnVariant::Outline => (theme.popover, theme.foreground, theme.muted),
        BtnVariant::Ghost => (theme.transparent, theme.foreground, theme.muted),
    };
    let (h, pad, fs, gap) = match size {
        BtnSize::Small => (px(28.), px(14.), px(12.), px(6.)),
        BtnSize::Medium => (px(32.), px(16.), px(14.), px(8.)),
        BtnSize::Large => (px(38.), px(20.), px(14.), px(8.)),
    };
    let inactive = disabled || loading;

    div()
        .id(id.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap(gap)
        .h(h)
        .px(pad)
        .rounded(px(8.))
        .text_size(fs)
        .font_weight(FontWeight::MEDIUM)
        // 禁用/加载：箭头光标 + 灰化；可用：手型 + 变体配色 + hover
        .when(inactive, |el| {
            el.cursor_default()
                .bg(theme.muted)
                .text_color(theme.muted_foreground)
                .border_1()
                .border_color(theme.border)
        })
        .when(!inactive, |el| {
            let mut el = el.cursor_pointer().bg(bg).text_color(fg);
            if variant == BtnVariant::Outline {
                el = el
                    .border_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(hover_bg).border_color(theme.sidebar_accent_foreground));
            } else if variant == BtnVariant::Ghost {
                el = el.hover(|s| s.bg(hover_bg));
            } else {
                el = el.hover(move |s| s.bg(hover_bg));
            }
            el
        })
        .on_click(move |ev, window, cx| {
            if !inactive {
                on_click(ev, window, cx);
            }
        })
        .when(loading, |el| el.child(Spinner::new().with_size(fs * 0.9)))
        .when(icon.is_some(), |el| el.child(Icon::new(icon.unwrap()).with_size(fs)))
        .child(label.to_string())
}

/// 页面标题区（各页顶部：大图标 + 标题 + 副标题，设计图统一元素）
pub fn page_header(icon: IconName, title: &'static str, subtitle: &'static str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    h_flex()
        .gap_3()
        .items_center()
        .pb_4()
        .child(
            // 浅蓝圆角底大图标
            h_flex()
                .size(px(48.))
                .rounded(px(12.))
                .justify_center()
                .bg(tks.primary_soft_bg)
                .child(Icon::new(icon).with_size(px(26.)).text_color(tks.primary_soft_fg)),
        )
        .child(
            v_flex()
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title.to_string()),
                )
                .child(div().text_sm().text_color(theme.muted_foreground).child(subtitle.to_string())),
        )
}
