//! SDK 管理页：搜索 + 筛选 + 公共 SDK 表格 + 右侧版本管理详情（切换/卸载/安装 + 进度）
//!
//! SDK 行数据与概览页共用（app.sdk_rows）；操作走两段式安装：输入版本 → resolve
//! （后台）→ fuzzy 内联确认 → install（后台，进度事件驱动进度条）；卸载内联确认条。

use crate::app::GuiApp;
use crate::components::{
    BtnSize, BtnVariant, SdkTableAction, btn, card, empty_state, filter_chip, fmt_bytes, pager, pill, pill_dot,
    sdk_icon, sdk_table, segmented_tabs,
};
use crate::data::{RemoteItem, SdkListItem, TaskId};
use crate::prefs::card_pad;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

// 安装对话框确认动作（Dialog 内确认按钮触发，根视图捕获后派发解析任务）
actions!(sdkm_gui, [SubmitInstallDialog]);

/// SDK 列表筛选
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkFilter {
    All,
    Installed,
    RemoteSupported,
    LocalOnly,
    Custom,
}

/// 详情页版本 tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionTab {
    Local,
    Remote,
}

/// 每页条数
const PAGE_SIZE: usize = 10;
/// 远程版本每页条数
const REMOTE_PAGE_SIZE: usize = 6;

/// SDK 管理页状态（SDK 行在 app.sdk_rows 共享；此处只管筛选/选中/详情交互）
pub struct SdkPageState {
    /// 搜索框
    pub search: Entity<InputState>,
    /// 安装版本输入框（弹窗用）
    pub install_input: Entity<InputState>,
    pub keyword: String,
    pub filter: SdkFilter,
    /// 选中项（app.sdk_rows 全量索引）
    pub selected: Option<usize>,
    /// 版本管理 tab（本地/远程）
    pub version_tab: VersionTab,
    /// 远程版本列表（按需加载）
    pub remote: Option<Vec<RemoteItem>>,
    /// 远程列表加载中
    pub remote_loading: bool,
    /// 操作执行中（禁用冲突按钮）
    pub busy: bool,
    /// 网络解析中（只禁用安装入口，不禁用其他操作）
    pub resolving: bool,
    /// 当前页（每页 8 条）
    pub page: usize,
    /// 远程版本搜索框
    pub remote_search: Entity<InputState>,
    pub remote_keyword: String,
    pub remote_page: usize,
}

impl SdkPageState {
    pub fn new(window: &mut Window, cx: &mut Context<GuiApp>, lang: &'static crate::i18n::Lang) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(lang.sdk.search_placeholder));
        let install_input = cx.new(|cx| InputState::new(window, cx).placeholder(lang.sdk.version_input_placeholder));
        Self {
            search,
            install_input,
            keyword: String::new(),
            filter: SdkFilter::All,
            selected: None,
            version_tab: VersionTab::Local,
            remote: None,
            remote_loading: false,
            busy: false,
            resolving: false,
            page: 0,
            remote_search: cx.new(|cx| InputState::new(window, cx).placeholder(lang.sdk.remote_search_placeholder)),
            remote_keyword: String::new(),
            remote_page: 0,
        }
    }

    /// 应用远程列表结果（当前选中匹配检查在 app.handle_event 完成）
    pub fn apply_remote(
        &mut self,
        result: Result<Vec<RemoteItem>, String>,
        window: &mut Window,
        cx: &mut Context<GuiApp>,
    ) {
        match result {
            Ok(items) => self.remote = Some(items),
            Err(_) => self.remote = Some(Vec::new()),
        }
        self.remote_loading = false;
        self.remote_page = 0;
        self.remote_keyword.clear();
        self.remote_search.update(cx, |s, cx| s.set_value("", window, cx));
    }

    /// 按筛选与关键字过滤（返回 (全量索引, 行) 对，保持索引对齐）
    fn filtered<'a>(&self, rows: &'a [SdkListItem]) -> Vec<(usize, &'a SdkListItem)> {
        let kw = self.keyword.to_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, it)| match self.filter {
                SdkFilter::All => true,
                SdkFilter::Installed => it.installed,
                SdkFilter::RemoteSupported => it.has_version_url,
                SdkFilter::LocalOnly => !it.has_version_url,
                SdkFilter::Custom => !it.is_builtin,
            })
            .filter(|(_, it)| kw.is_empty() || it.name.to_lowercase().contains(&kw))
            .collect()
    }

    fn count(&self, rows: Option<&Vec<SdkListItem>>, f: SdkFilter) -> usize {
        rows.map(|rows| {
            rows.iter()
                .filter(|it| match f {
                    SdkFilter::All => true,
                    SdkFilter::Installed => it.installed,
                    SdkFilter::RemoteSupported => it.has_version_url,
                    SdkFilter::LocalOnly => !it.has_version_url,
                    SdkFilter::Custom => !it.is_builtin,
                })
                .count()
        })
        .unwrap_or(0)
    }
}

/// 渲染 SDK 管理页（顶部操作行 + 筛选 + 表格 + 右详情）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let state = &app.sdk_page;

    // ── 顶部操作行：搜索 + 刷新 + 安装 SDK ──
    let toolbar = h_flex()
        .gap_3()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(&state.search).prefix(Icon::new(IconName::Search).small())),
        )
        .child(btn(
            "sdk-refresh",
            BtnVariant::Ghost,
            BtnSize::Medium,
            "",
            Some(IconName::RefreshCw),
            false,
            app.sdk_page.busy,
            cx.listener(|app, _, _, cx| {
                app.dispatch(TaskId::SdkListRefresh, cx);
            }),
            cx,
        ))
        .child(btn(
            "sdk-add-custom",
            BtnVariant::Primary,
            BtnSize::Medium,
            t.sdk.btn_add_custom,
            Some(IconName::Plus),
            false,
            false,
            cx.listener(open_add_custom_dialog),
            cx,
        ));

    // ── 筛选 chips ──
    let chips = filter_tabs(app, cx);

    // ── 表格（公共组件，行点击选中）──
    let table_card = card("sdk-table", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .flex_1()
        .min_w_0()
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
                empty_state(IconName::SearchX, t.common.empty_list, t.sdk.search_placeholder, cx).into_any_element()
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
                    .child(sdk_table(app, slice, SdkTableAction::Select, cx))
                    .when(pages > 1, |el| {
                        el.child(crate::components::pager(
                            "sdk-pager",
                            page,
                            pages,
                            t.common.btn_prev,
                            t.common.btn_next,
                            cx,
                            |app, cx| {
                                app.sdk_page.page = app.sdk_page.page.saturating_sub(1);
                                cx.notify();
                            },
                            |app, cx| {
                                app.sdk_page.page += 1;
                                cx.notify();
                            },
                        ))
                    })
                    .into_any_element()
            }
        });

    // ── 右详情 ──
    let detail_card = card("sdk-detail", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .w(px(340.))
        .flex_shrink_0()
        .child(
            match state.selected.and_then(|i| app.sdk_rows.as_ref().and_then(|r| r.get(i))) {
                None => empty_state(IconName::MousePointerClick, t.common.empty_list, "", cx).into_any_element(),
                Some(row) => sdk_detail(app, row, cx).into_any_element(),
            },
        );

    v_flex()
        .gap_3()
        .child(toolbar)
        .child(h_flex().gap_4().items_start().child(table_card).child(detail_card))
}

/// 筛选 chips 行（真实计数 + 选中蓝底）
fn filter_tabs(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;

    let defs = [
        (SdkFilter::All, t.overview.filter_all),
        (SdkFilter::Installed, t.overview.filter_installed),
        (SdkFilter::RemoteSupported, t.overview.filter_remote),
        (SdkFilter::LocalOnly, t.overview.filter_local_only),
        (SdkFilter::Custom, t.overview.filter_custom),
    ];

    defs.iter().fold(h_flex().gap_2(), |acc, (f, label)| {
        let f = *f;
        let selected = app.sdk_page.filter == f;
        let count = app.sdk_page.count(app.sdk_rows.as_ref(), f);
        acc.child(filter_chip(
            ElementId::Name(format!("sdk-tab-{:?}", f).into()),
            format!("{label} ({count})"),
            selected,
            cx.listener(move |app, _, _, cx| {
                app.sdk_page.filter = f;
                app.sdk_page.page = 0;
                cx.notify();
            }),
            cx,
        ))
    })
}

/// 右侧详情面板（行数据来自共享 sdk_rows）
fn sdk_detail(app: &GuiApp, it: &SdkListItem, cx: &Context<GuiApp>) -> AnyElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);

    let mut col = v_flex().gap_3();

    // ── SDK 头部 ──
    let (tag_bg, tag_fg) = if it.installed {
        (tks.success_bg, tks.success_fg)
    } else {
        (theme.muted, theme.muted_foreground)
    };
    col = col.child(
        h_flex()
            .gap_3()
            .items_center()
            .child(sdk_icon(&it.name, 44., cx))
            .child(
                v_flex()
                    .gap_0p5()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(it.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(crate::i18n::sdk_desc(&it.name, app.prefs.lang)),
                    ),
            )
            .child(div().ml_auto().child(pill_dot(
                if it.installed {
                    t.overview.status_installed
                } else {
                    t.overview.status_not_installed
                },
                if it.installed {
                    theme.success
                } else {
                    theme.muted_foreground
                },
                tag_bg,
                tag_fg,
            ))),
    );

    // ── 当前激活版本卡（版本号 + 全局 tag；切换入口在版本行）──
    col = col.child(
        v_flex()
            .gap_1p5()
            .rounded(px(8.))
            .bg(theme.muted)
            .p_3()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t.sdk.current_active.to_string()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(it.current.clone().unwrap_or_else(|| "—".into())),
                    )
                    .when(it.current.is_some(), |el| {
                        el.child(pill(t.sdk.scope_global, tks.primary_soft_bg, tks.primary_soft_fg))
                    }),
            )
            .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(format!(
                "{} {}",
                t.sdk.install_path,
                it.current.as_ref().map(|v| format!("{}/{}", it.name, v)).unwrap_or_default()
            ))),
    );

    // ── 本地版本数 / 占用空间（size 经 SizeCache，OverviewRefresh 已算好缓存）──
    let disk = app
        .overview
        .sizes
        .get(&it.name)
        .filter(|sz| **sz > 0)
        .map(|sz| fmt_bytes(*sz))
        .unwrap_or_else(|| "—".into());
    col = col.child(
        h_flex()
            .gap_3()
            .child(info_tile(
                "tile-count",
                IconName::Layers,
                t.sdk.local_version_count,
                it.versions.len().to_string(),
                t.sdk.local_version_count_desc,
                cx,
            ))
            .child(info_tile(
                "tile-disk",
                IconName::HardDrive,
                t.sdk.disk_usage,
                disk,
                t.sdk.disk_usage_desc,
                cx,
            )),
    );

    // ── 版本管理 segmented tabs ──
    col = col.child(
        v_flex()
            .gap_2()
            .child(segmented_tabs(
                "version-tabs",
                &[("local", t.sdk.tab_installed), ("remote", t.sdk.tab_remote)],
                match app.sdk_page.version_tab {
                    VersionTab::Local => 0,
                    VersionTab::Remote => 1,
                },
                cx,
                |app, i, cx| {
                    app.sdk_page.version_tab = match i {
                        1 => VersionTab::Remote,
                        _ => VersionTab::Local,
                    };
                    app.sdk_page.remote_page = 0;
                    // 首次切远程 tab 且 SDK 支持时拉取远程列表
                    if app.sdk_page.version_tab == VersionTab::Remote
                        && app.sdk_page.remote.is_none()
                        && !app.sdk_page.remote_loading
                    {
                        if let Some(name) = selected_sdk_name(app) {
                            let supported = app
                                .sdk_rows
                                .iter()
                                .flatten()
                                .find(|it| it.name == name)
                                .map(|it| it.has_version_url)
                                .unwrap_or(false);
                            if supported {
                                app.sdk_page.remote_loading = true;
                                app.dispatch(TaskId::SdkRemoteList { name }, cx);
                                return;
                            }
                        }
                        // 不支持远程：空态
                        app.sdk_page.remote = Some(Vec::new());
                    }
                    cx.notify();
                },
            ))
            .child(match app.sdk_page.version_tab {
                VersionTab::Local => version_rows(app, it, cx).into_any_element(),
                VersionTab::Remote => remote_rows(app, it, cx).into_any_element(),
            }),
    );

    // ── 安装新版本 ──
    col = col.child(btn(
        "install-new",
        BtnVariant::Primary,
        BtnSize::Large,
        t.sdk.btn_install_version,
        Some(IconName::Download),
        app.sdk_page.busy || app.sdk_page.resolving,
        app.sdk_page.resolving,
        cx.listener(open_install_dialog),
        cx,
    ));

    col.into_any_element()
}

/// 选中 SDK 名（从共享数据）
fn selected_sdk_name(app: &GuiApp) -> Option<String> {
    app.sdk_page
        .selected
        .and_then(|i| app.sdk_rows.as_ref().and_then(|rows| rows.get(i)))
        .map(|it| it.name.clone())
}

/// 详情小信息块（本地版本数 / 占用空间）
fn info_tile(
    id: &'static str,
    icon: IconName,
    label: &str,
    value: String,
    sub: &str,
    cx: &Context<GuiApp>,
) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    v_flex()
        .id(ElementId::Name(id.into()))
        .flex_1()
        .gap_1()
        .rounded(px(8.))
        .border_1()
        .border_color(theme.border)
        .p_2p5()
        .child(
            // 图标 + 标签（设计图 p2：蓝图标 + 灰标签）
            h_flex()
                .gap_1p5()
                .items_center()
                .child(Icon::new(icon).small().text_color(tks.primary_soft_fg))
                .child(div().text_xs().text_color(theme.muted_foreground).child(label.to_string())),
        )
        .child(
            div()
                .text_size(px(18.))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.foreground)
                .child(value),
        )
        .child(div().text_xs().text_color(theme.muted_foreground).child(sub.to_string()))
}

/// 本地版本行（版本号 + 激活 tag + 切换/卸载）
fn version_rows(app: &GuiApp, it: &SdkListItem, cx: &Context<GuiApp>) -> AnyElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);

    if it.versions.is_empty() {
        return empty_state(IconName::PackageOpen, t.sdk.local_empty, t.sdk.local_empty_desc, cx).into_any_element();
    }

    let rows = it.versions.iter().enumerate().fold(v_flex().gap_1(), |acc, (i, ver)| {
        let active = it.current.as_deref() == Some(ver.as_str());
        // 拷贝值进 listener（listener 要求 'static；两个按钮各自持有一份）
        let (sw_name, sw_ver) = (it.name.clone(), ver.clone());
        let (un_name, un_ver) = (it.name.clone(), ver.clone());
        acc.child(
            h_flex()
                .id(ElementId::NamedInteger("ver-row".into(), i as u64))
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .hover(|s| s.bg(theme.muted))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(ver.clone()),
                )
                .when(active, |el| el.child(pill(t.sdk.active_badge, tks.success_bg, tks.success_fg)))
                .child(
                    h_flex()
                        .ml_auto()
                        .gap_1p5()
                        .child(btn(
                            ElementId::NamedInteger("ver-switch".into(), i as u64),
                            BtnVariant::Outline,
                            BtnSize::Small,
                            t.sdk.btn_switch,
                            None,
                            app.sdk_page.busy || active,
                            false,
                            cx.listener(move |app, _, _, cx| {
                                app.dispatch(
                                    TaskId::SdkSwitch {
                                        name: sw_name.clone(),
                                        version: sw_ver.clone(),
                                    },
                                    cx,
                                );
                            }),
                            cx,
                        ))
                        .child(btn(
                            ElementId::NamedInteger("ver-un".into(), i as u64),
                            BtnVariant::Danger,
                            BtnSize::Small,
                            t.sdk.btn_uninstall,
                            None,
                            app.sdk_page.busy || active,
                            false,
                            cx.listener(move |app, _, window, cx| {
                                open_uninstall_confirm(app, window, cx, un_name.clone(), un_ver.clone());
                            }),
                            cx,
                        )),
                ),
        )
    });

    rows.into_any_element()
}

/// 远程版本行（版本号 + 状态 + 安装按钮）
fn remote_rows(app: &GuiApp, it: &SdkListItem, cx: &Context<GuiApp>) -> AnyElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);

    // 不支持远程发现的 SDK
    if !it.has_version_url {
        return empty_state(IconName::CloudOff, t.sdk.remote_empty, t.sdk.remote_empty_desc, cx).into_any_element();
    }
    // 加载中（列表区域居中 loading，文案 i18n）
    if app.sdk_page.remote_loading {
        return v_flex()
            .flex_1()
            .min_h(px(120.))
            .items_center()
            .justify_center()
            .child(
                h_flex().gap_2().items_center().child(Spinner::new().small()).child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(t.common.loading.to_string()),
                ),
            )
            .into_any_element();
    }
    let Some(remote) = &app.sdk_page.remote else {
        return div().into_any_element();
    };
    if remote.is_empty() {
        return empty_state(IconName::CloudOff, t.common.empty_list, t.sdk.remote_empty_desc, cx).into_any_element();
    }

    let kw = app.sdk_page.remote_keyword.to_lowercase();
    let filtered: Vec<&RemoteItem> = remote
        .iter()
        .filter(|r| kw.is_empty() || r.full_version.to_lowercase().contains(&kw))
        .collect();
    let total = filtered.len();
    let pages = total.div_ceil(REMOTE_PAGE_SIZE).max(1);
    let page = app.sdk_page.remote_page.min(pages - 1);
    let start = page * REMOTE_PAGE_SIZE;
    let slice = &filtered[start..(start + REMOTE_PAGE_SIZE).min(total)];

    let mut body = v_flex().gap_2().child(Input::new(&app.sdk_page.remote_search).small());
    if total == 0 {
        body = body.child(empty_state(IconName::SearchX, t.common.no_results, t.sdk.remote_empty_desc, cx));
    } else {
        let rows = slice.iter().enumerate().fold(v_flex().gap_1(), |acc, (i, r)| {
            let name = it.name.clone();
            let version = r.full_version.clone();
            let installed = r.installed;
            acc.child(
                h_flex()
                    .id(ElementId::NamedInteger("rver-row".into(), i as u64))
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(r.full_version.clone()),
                    )
                    .when(r.active, |el| {
                        el.child(pill(t.sdk.active_badge, tks.success_bg, tks.success_fg))
                    })
                    .when(installed && !r.active, |el| {
                        el.child(pill(t.overview.status_installed, theme.muted, theme.muted_foreground))
                    })
                    .child(div().ml_auto().child(btn(
                        ElementId::NamedInteger("rver-install".into(), i as u64),
                        BtnVariant::Primary,
                        BtnSize::Small,
                        if installed {
                            t.overview.status_installed
                        } else {
                            t.sdk.btn_confirm_install
                        },
                        None,
                        app.sdk_page.busy || app.sdk_page.resolving || installed,
                        app.sdk_page.resolving,
                        cx.listener(move |app, _, _window, cx| {
                            app.dispatch(
                                TaskId::SdkInstallResolve {
                                    name: name.clone(),
                                    version: version.clone(),
                                },
                                cx,
                            );
                        }),
                        cx,
                    ))),
            )
        });
        body = body.child(rows);
        if pages > 1 {
            body = body.child(pager(
                "remote-pager",
                page,
                pages,
                t.common.btn_prev,
                t.common.btn_next,
                cx,
                |app, cx| {
                    app.sdk_page.remote_page = app.sdk_page.remote_page.saturating_sub(1);
                    cx.notify();
                },
                |app, cx| {
                    app.sdk_page.remote_page += 1;
                    cx.notify();
                },
            ));
        }
    }
    body.into_any_element()
}

/// 顶部/底部安装按钮：弹安装输入 Dialog（确认经 action 冒泡到根视图派发解析）
fn open_install_dialog(app: &mut GuiApp, _: &ClickEvent, window: &mut Window, cx: &mut Context<GuiApp>) {
    let t = app.lang;
    // 清空上次输入
    app.sdk_page.install_input.update(cx, |state, cx| {
        state.set_value("", window, cx);
    });
    let input = app.sdk_page.install_input.clone();
    window.open_alert_dialog(cx, move |alert, _, _cx| {
        let input = input.clone();
        alert
            .title(t.sdk.btn_install_version.to_string())
            .content(move |content, _, _cx| {
                content.child(
                    v_flex()
                        .gap_3()
                        .p_1()
                        .w_full()
                        .child(div().text_sm().child(t.sdk.version_input_label.to_string()))
                        .child(Input::new(&input)),
                )
            })
            .confirm()
            .ok_text(t.sdk.btn_confirm_install)
            .cancel_text(t.common.btn_cancel)
            .on_ok(move |_, window, cx| {
                window.dispatch_action(Box::new(SubmitInstallDialog), cx);
                true
            })
    });
}

/// 卸载确认弹窗（统一居中弹窗；on_ok 直接派发卸载任务）
fn open_uninstall_confirm(
    app: &mut GuiApp,
    window: &mut Window,
    cx: &mut Context<GuiApp>,
    name: String,
    version: String,
) {
    let t = app.lang;
    let tx = app.tx.clone();
    let title = format!("{}: {name} {version}", t.sdk.uninstall_confirm_title);
    let desc = t.sdk.uninstall_confirm_desc;
    let ok_text = t.sdk.btn_uninstall;
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc)
            .confirm()
            .ok_text(ok_text)
            .cancel_text(t.common.btn_cancel)
            .ok_variant(ButtonVariant::Danger)
            .on_ok({
                let tx = tx.clone();
                let name = name.clone();
                let version = version.clone();
                move |_, _, _| {
                    crate::data::spawn_task(
                        tx.clone(),
                        TaskId::SdkUninstall {
                            name: name.clone(),
                            version: version.clone(),
                        },
                    );
                    true
                }
            })
    });
}

/// 添加自定义 SDK 表单弹窗（名称必填；GitHub Releases 只填名称 + version_url 即可）
fn open_add_custom_dialog(app: &mut GuiApp, _: &ClickEvent, window: &mut Window, cx: &mut Context<GuiApp>) {
    let t = app.lang;
    let tx = app.tx.clone();
    // 表单输入（弹窗局部创建）
    let f_name = cx.new(|cx| InputState::new(window, cx).placeholder(t.sdk.form_name_ph));
    let f_version_url = cx.new(|cx| InputState::new(window, cx).placeholder(t.sdk.form_version_url_ph));
    let f_download_url = cx.new(|cx| InputState::new(window, cx).placeholder(t.sdk.form_download_url_ph));
    let f_bin_dir = cx.new(|cx| InputState::new(window, cx).placeholder(t.sdk.form_bin_dir_ph));

    let (n, v, d, b) = (f_name.clone(), f_version_url.clone(), f_download_url.clone(), f_bin_dir.clone());
    window.open_alert_dialog(cx, move |alert, _, _cx| {
        let content_fields = (n.clone(), v.clone(), d.clone(), b.clone());
        alert
            .width(px(560.))
            .title(t.sdk.btn_add_custom.to_string())
            .content({
                let (n, v, d, b) = content_fields;
                move |content, _, _cx| {
                    content.child(
                        v_flex()
                            .gap_3()
                            .p_1()
                            .w_full()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui_kit::rgb(0x64748b))
                                    .child(t.sdk.form_hint.to_string()),
                            )
                            .child(form_field(t.sdk.form_name, Input::new(&n)))
                            .child(form_field(t.sdk.form_version_url, Input::new(&v)))
                            .child(form_field(t.sdk.form_download_url, Input::new(&d)))
                            .child(form_field(t.sdk.form_bin_dir, Input::new(&b))),
                    )
                }
            })
            .confirm()
            .ok_text(t.sdk.form_submit)
            .cancel_text(t.common.btn_cancel)
            .on_ok({
                let tx = tx.clone();
                let (n, v, d, b) = (n.clone(), v.clone(), d.clone(), b.clone());
                move |_, _, cx| {
                    let (n, v, d, b) = (
                        n.read(cx).value().trim().to_string(),
                        v.read(cx).value().trim().to_string(),
                        d.read(cx).value().trim().to_string(),
                        b.read(cx).value().trim().to_string(),
                    );
                    if n.is_empty() {
                        return false;
                    }
                    crate::data::spawn_task(
                        tx.clone(),
                        TaskId::ConfigAddSdk {
                            name: n,
                            version_url: non_empty(v),
                            download_url: non_empty(d),
                            bin_dir: non_empty(b),
                        },
                    );
                    true
                }
            })
    });
}

/// 表单字段行（label + 控件）
fn form_field(label: &str, control: Input) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label.to_string()))
        .child(control)
}

/// 空字符串转 None（表单可选项）
fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}
