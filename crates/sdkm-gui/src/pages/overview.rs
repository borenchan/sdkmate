//! 概览页：统计卡片 + SDK 表格（搜索/筛选）+ 快速操作 + 存储占用环形图 + 系统信息
//!
//! SDK 行数据与 SDK 管理页共用（app.sdk_rows），表格组件复用 components::sdk_table。

use crate::app::GuiApp;
use crate::components::{
    SdkTableAction, action_tile, card, card_header, empty_state, filter_chip, fmt_bytes, pager, sdk_table, stat_card,
};
use crate::data::SdkListItem;
use crate::prefs::card_pad;
use gpui_kit::assets::IconName;
use gpui_kit::component::chart::PieChart;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 每页条数
const PAGE_SIZE: usize = 8;

/// 列表筛选类型（chips）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewFilter {
    All,
    Installed,
    RemoteSupported,
    LocalOnly,
    Custom,
}

/// 概览页状态（SDK 行在 app.sdk_rows 共享；此处只管筛选与展示）
pub struct OverviewState {
    /// 搜索框
    pub search: Entity<InputState>,
    /// 当前筛选关键字
    pub keyword: String,
    pub filter: OverviewFilter,
    /// sdk_name -> 存储字节数（后台线程算完一次性回传）
    pub sizes: std::collections::HashMap<String, u64>,
    pub total_size: u64,
    /// 当前页（每页 10 条）
    pub page: usize,
    /// 初始化状态（config + store 目录存在性，真实检查）
    pub init_ok: bool,
}

impl OverviewState {
    pub fn new(window: &mut Window, cx: &mut Context<GuiApp>, lang: &'static crate::i18n::Lang) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(lang.overview.search_placeholder));
        Self {
            search,
            keyword: String::new(),
            filter: OverviewFilter::All,
            sizes: Default::default(),
            total_size: 0,
            page: 0,
            init_ok: true,
        }
    }

    /// 按筛选与关键字过滤（返回 (全量索引, 行) 对，保持索引对齐）
    pub fn filtered<'a>(&self, rows: &'a [SdkListItem]) -> Vec<(usize, &'a SdkListItem)> {
        let kw = self.keyword.to_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, r)| match self.filter {
                OverviewFilter::All => true,
                OverviewFilter::Installed => r.installed,
                OverviewFilter::RemoteSupported => r.has_version_url,
                OverviewFilter::LocalOnly => !r.has_version_url,
                OverviewFilter::Custom => !r.is_builtin,
            })
            .filter(|(_, r)| kw.is_empty() || r.name.to_lowercase().contains(&kw))
            .collect()
    }

    fn count(&self, rows: Option<&Vec<SdkListItem>>, f: OverviewFilter) -> usize {
        rows.map(|rows| {
            rows.iter()
                .filter(|r| match f {
                    OverviewFilter::All => true,
                    OverviewFilter::Installed => r.installed,
                    OverviewFilter::RemoteSupported => r.has_version_url,
                    OverviewFilter::LocalOnly => !r.has_version_url,
                    OverviewFilter::Custom => !r.is_builtin,
                })
                .count()
        })
        .unwrap_or(0)
    }
}

/// 渲染概览页（左侧主列 + 右侧栏）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let state = &app.overview;

    let v = state.count(app.sdk_rows.as_ref(), OverviewFilter::Installed);
    let a = app
        .sdk_rows
        .as_ref()
        .map(|rows| rows.iter().filter(|r| r.current.is_some()).count())
        .unwrap_or(0);
    let storage = if state.total_size > 0 {
        fmt_bytes(state.total_size)
    } else {
        "—".into()
    };

    // ── 统计卡片行（flex_1 均分弹性；图标蓝=数据/绿=健康，soft 底随主题）──
    let stats = h_flex()
        .gap_4()
        .child(stat_card(
            "stat-installed",
            IconName::Package,
            tks.icon_blue,
            tks.primary_soft_bg,
            t.overview.installed_sdks,
            &v.to_string(),
            t.overview.unit_tools,
            cx,
        ))
        .child(stat_card(
            "stat-active",
            IconName::CircleCheck,
            tks.icon_green,
            tks.success_bg,
            t.overview.active_versions,
            &a.to_string(),
            t.overview.unit_sdks,
            cx,
        ))
        .child(stat_card(
            "stat-storage",
            IconName::HardDrive,
            tks.icon_blue,
            tks.primary_soft_bg,
            t.overview.storage_used,
            &storage,
            t.overview.unit_total,
            cx,
        ))
        .child(init_stat_card(
            "stat-init",
            IconName::ShieldCheck,
            tks.icon_green,
            t.overview.init_status,
            if state.init_ok {
                t.overview.status_ok
            } else {
                t.overview.status_uninit
            },
            if state.init_ok {
                t.overview.env_configured
            } else {
                t.overview.env_not_configured
            },
            (tks.success_bg, tks.success_fg),
            cx,
        ));

    // ── SDK 列表卡（标题 + 搜索框 + chips + 公共表格）──
    let header_row = h_flex()
        .items_center()
        .justify_between()
        .pb_2()
        .child(card_header(t.overview.sdk_list, cx))
        .child(
            div()
                .w(px(260.))
                .child(Input::new(&state.search).prefix(Icon::new(IconName::Search).small())),
        );

    let chips = filter_chips(app, cx);

    let list_card = card("overview-list", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .flex_1()
        .child(header_row)
        .child(chips)
        .child(match app.sdk_rows.as_ref() {
            None => v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(t.common.loading.to_string()),
                )
                .into_any_element(),
            Some(rows) if state.filtered(rows).is_empty() => {
                empty_state(IconName::SearchX, t.overview.empty_no_sdks, t.overview.empty_no_sdks_desc, cx)
                    .into_any_element()
            }
            Some(rows) => {
                let filtered = state.filtered(rows);
                let total = filtered.len();
                let pages = total.div_ceil(PAGE_SIZE).max(1);
                let page = state.page.min(pages - 1);
                let start = page * PAGE_SIZE;
                let slice = &filtered[start..(start + PAGE_SIZE).min(total)];
                v_flex()
                    .gap_2()
                    .child(sdk_table(app, slice, SdkTableAction::JumpAndSelect, cx))
                    .when(pages > 1, |el| {
                        el.child(pager(
                            "overview-pager",
                            page,
                            pages,
                            t.common.btn_prev,
                            t.common.btn_next,
                            cx,
                            |app, cx| {
                                app.overview.page = app.overview.page.saturating_sub(1);
                                cx.notify();
                            },
                            |app, cx| {
                                app.overview.page += 1;
                                cx.notify();
                            },
                        ))
                    })
                    .into_any_element()
            }
        });

    // ── 主列 ──
    let main_col = v_flex().flex_1().min_w_0().gap_4().child(stats).child(list_card);

    // ── 右侧栏 ──
    let side_col = v_flex()
        .w(px(340.))
        .flex_shrink_0()
        .gap_4()
        .child(quick_actions(app, cx))
        .child(storage_card(app, cx))
        .child(system_info_card(app, cx));

    h_flex().gap_4().items_start().child(main_col).child(side_col)
}

/// 筛选 chips 行（真实计数 + 选中蓝底）
fn filter_chips(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;

    let defs = [
        (OverviewFilter::All, t.overview.filter_all),
        (OverviewFilter::Installed, t.overview.filter_installed),
        (OverviewFilter::RemoteSupported, t.overview.filter_remote),
        (OverviewFilter::LocalOnly, t.overview.filter_local_only),
        (OverviewFilter::Custom, t.overview.filter_custom),
    ];

    defs.iter().fold(h_flex().gap_2(), |acc, (f, label)| {
        let f = *f;
        let selected = app.overview.filter == f;
        let count = app.overview.count(app.sdk_rows.as_ref(), f);
        acc.child(filter_chip(
            ElementId::Name(format!("chip-{:?}", f).into()),
            format!("{label} ({count})"),
            selected,
            cx.listener(move |app, _, _, cx| {
                app.overview.filter = f;
                app.overview.page = 0;
                cx.notify();
            }),
            cx,
        ))
    })
}

/// 初始化状态卡（与统计卡同构，但"正常"为绿色半圆角 pill，按设计图）
#[allow(clippy::too_many_arguments)]
fn init_stat_card(
    id: impl Into<ElementId>,
    icon: IconName,
    icon_fg: Hsla,
    label: &str,
    badge_text: &str,
    sub: &str,
    badge_colors: (Hsla, Hsla),
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    card(id, cx)
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p_3()
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .min_w_0()
                .child(
                    h_flex()
                        .size(px(30.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .justify_center()
                        .bg(tks.primary_soft_bg)
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
                .text_color(badge_colors.1)
                .truncate()
                .child(badge_text.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .truncate()
                .child(sub.to_string()),
        )
}

/// 快速操作卡（2×2 等宽网格）
fn quick_actions(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    card("quick-actions", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .child(card_header(t.overview.quick_actions, cx))
        .child(
            // 左右两列各 flex_1，列内 tile 全宽——四块等宽对齐，随窗口弹性伸缩
            h_flex()
                .gap_3()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_3()
                        .child(action_tile(
                            "qa-install",
                            IconName::Plus,
                            t.overview.qa_install,
                            t.overview.qa_install_desc,
                            cx.listener(|app, _, _, cx| app.navigate(crate::app::Page::Sdk, cx)),
                            cx,
                        ))
                        .child(action_tile(
                            "qa-update",
                            IconName::RefreshCw,
                            t.overview.qa_update,
                            t.overview.qa_update_desc,
                            cx.listener(|app, _, _, cx| app.navigate(crate::app::Page::Diagnostics, cx)),
                            cx,
                        )),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_3()
                        .child(action_tile(
                            "qa-projects",
                            IconName::FolderOpen,
                            t.overview.qa_projects,
                            t.overview.qa_projects_desc,
                            cx.listener(|app, _, _, cx| app.navigate(crate::app::Page::Projects, cx)),
                            cx,
                        ))
                        .child(action_tile(
                            "qa-diagnostics",
                            IconName::Activity,
                            t.overview.qa_diagnostics,
                            t.overview.qa_diagnostics_desc,
                            cx.listener(|app, _, _, cx| app.navigate(crate::app::Page::Diagnostics, cx)),
                            cx,
                        )),
                ),
        )
}

/// 存储占用卡：环形图（PieChart）+ 中心总量 + 右侧图例，与设计图一致
fn storage_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let colors = crate::theme::chart_colors(crate::theme::is_dark(cx));

    let total = app.overview.total_size;
    let mut items: Vec<(String, u64)> = app
        .overview
        .sizes
        .iter()
        .map(|item| (item.0.clone(), *item.1))
        .filter(|(_, sz)| *sz > 0)
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));

    let card_el = card("storage", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .child(card_header(t.overview.storage_chart, cx));

    if items.is_empty() {
        return card_el.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(t.common.empty_list.to_string()),
        );
    }

    // top3 + 其他（与设计图 4 分类一致），每项预配颜色供 PieChart 闭包取用
    let other: u64 = items.iter().skip(3).map(|(_, sz)| sz).sum();
    let mut slices: Vec<(String, u64, Hsla)> = items
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, (name, sz))| (name.clone(), *sz, colors[i % colors.len()]))
        .collect();
    if other > 0 {
        slices.push((t.overview.storage_other.to_string(), other, colors[3]));
    }

    let pie = PieChart::new(slices.clone())
        .id("storage-pie")
        .value(|s| s.1 as f32)
        .color(|s| s.2)
        .inner_radius(52.)
        .outer_radius(74.)
        .name(t.overview.storage_chart);

    let legend = slices.iter().fold(v_flex().gap_1p5(), |acc, (name, sz, color)| {
        acc.child(
            h_flex()
                .items_center()
                .gap_2()
                .child(div().size_2().rounded_full().bg(*color))
                .child(
                    div()
                        .text_sm()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.foreground)
                        .child(name.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground)
                        .child(fmt_bytes(*sz)),
                ),
        )
    });

    card_el.child(
        h_flex()
            .items_center()
            .gap_3()
            .child(
                // 环形图 + 中心总量文字（overlay 居中）
                div().relative().size(px(148.)).flex_shrink_0().child(pie).child(
                    v_flex()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(fmt_bytes(total)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t.overview.unit_total.to_string()),
                        ),
                ),
            )
            .child(legend),
    )
}

/// 系统信息卡行：label 左 + value 右对齐小字（长路径截断，绝不溢出卡片）
fn sys_kv_row(label: &str, value: &str, cx: &Context<GuiApp>) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .items_start()
        .justify_between()
        .gap_3()
        .py_1()
        .child(
            div()
                .text_xs()
                .flex_shrink_0()
                .text_color(theme.muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(theme.foreground)
                .text_align(gpui_kit::TextAlign::Right)
                .truncate()
                .child(value.to_string()),
        )
}

/// 系统信息卡（真实数据：OS/架构/版本/路径）
fn system_info_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;

    let info = &app.sys_info;
    card("sys-info", cx)
        .flex()
        .flex_col()
        .min_w_0()
        .p(px(card_pad()))
        .gap_1()
        .child(card_header(t.overview.system_info, cx))
        .child(sys_kv_row(t.overview.info_os, &info.os, cx))
        .child(sys_kv_row(t.overview.info_arch, &info.arch, cx))
        .child(sys_kv_row(t.overview.info_version, &info.version, cx))
        .child(sys_kv_row(
            t.overview.info_privilege,
            if cfg!(windows) && crate::platform::is_elevated() {
                t.overview.value_admin
            } else {
                t.overview.value_user
            },
            cx,
        ))
        .child(sys_kv_row(t.overview.info_home, &info.home_path, cx))
}
