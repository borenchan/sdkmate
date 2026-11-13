//! 设置页：顶部 tab 锚点 + 纵向堆叠分区（p4 设计图）。
//!
//! - tab 栏只做锚点导航，内容全部纵向排列，默认展示第一个 tab，向下滚动看到其余分区；
//! - 界面偏好即时写入 `gui.toml`（语言/主题/默认页面/密度）；
//! - SDKM 环境 / SDK 源为“可选保存”：点保存才写 config.toml，可重新加载放弃修改。

use crate::app::{
    GuiApp, SetDefaultDiagnostics, SetDefaultOverview, SetDefaultProjects, SetDefaultSdk, SetDefaultSettings,
};
use crate::components::{BtnSize, BtnVariant, btn, card, empty_state, pager, pill, sdk_icon};
use crate::data::{SdkSourceEntry, TaskId};
use crate::prefs::{GuiDensity, GuiLang, GuiThemeMode, card_pad, set_density};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, DropdownButton};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sdkm_util::builtin::find_seed;

/// 每页条数
const PAGE_SIZE: usize = 10;

/// SDK 源草稿（弹窗修改，列表底部统一保存）
#[derive(Debug, Clone)]
pub struct SourceDraft {
    pub name: String,
    pub is_builtin: bool,
    pub version_url: String,
    pub version_fallback_url: String,
    pub download_url: String,
    pub download_fallback_url: String,
}

/// 设置页状态
pub struct SettingsState {
    /// 网络表单（ConfigLoad 回填）
    pub proxy: Entity<InputState>,
    pub timeout: Entity<InputState>,
    pub ttl: Entity<InputState>,
    pub token: Entity<InputState>,
    pub mirror: Entity<InputState>,
    pub ssl_verify: bool,
    /// 表单是否已回填（首次进入时加载）
    pub loaded: bool,
    /// 符号链接目录（只读展示，浏览后写 config.toml）
    pub symlink_dir: String,
    pub picking_symlink: bool,
    /// SDK 源草稿（4 个地址）
    pub sources: Vec<SourceDraft>,
    pub source_search: Entity<InputState>,
    pub source_keyword: String,
    pub source_page: usize,
    /// 源编辑弹窗缓冲
    pub edit_v: Entity<InputState>,
    pub edit_vfb: Entity<InputState>,
    pub edit_d: Entity<InputState>,
    pub edit_dfb: Entity<InputState>,
    /// 分区滚动句柄（tab 点击锚点跳转）
    pub scroll: ScrollHandle,
    /// 当前 tab 高亮（只影响样式，内容始终全部渲染）
    pub tab: usize,
    /// 自定义 SDK 新增表单展开
    pub add_open: bool,
    pub new_name: Entity<InputState>,
    pub new_version_url: Entity<InputState>,
    pub new_download_url: Entity<InputState>,
    pub new_bin_dir: Entity<InputState>,
}

impl SettingsState {
    pub fn new(window: &mut Window, cx: &mut Context<GuiApp>, lang: &'static crate::i18n::Lang) -> Self {
        let proxy = cx.new(|cx| InputState::new(window, cx).placeholder(lang.settings.proxy_placeholder));
        let timeout = cx.new(|cx| InputState::new(window, cx));
        let ttl = cx.new(|cx| InputState::new(window, cx));
        let token = cx.new(|cx| InputState::new(window, cx));
        let mirror = cx.new(|cx| InputState::new(window, cx));
        let source_search = cx.new(|cx| InputState::new(window, cx).placeholder("SDK..."));
        let edit_v = cx.new(|cx| InputState::new(window, cx));
        let edit_vfb = cx.new(|cx| InputState::new(window, cx));
        let edit_d = cx.new(|cx| InputState::new(window, cx));
        let edit_dfb = cx.new(|cx| InputState::new(window, cx));
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder(lang.settings.new_name_placeholder));
        let new_version_url = cx.new(|cx| InputState::new(window, cx).placeholder("https://.../releases"));
        let new_download_url = cx.new(|cx| InputState::new(window, cx).placeholder("https://.../{version}/..."));
        let new_bin_dir = cx.new(|cx| InputState::new(window, cx).placeholder("bin"));
        Self {
            proxy,
            timeout,
            ttl,
            token,
            mirror,
            ssl_verify: true,
            loaded: false,
            symlink_dir: String::new(),
            picking_symlink: false,
            sources: Vec::new(),
            source_search,
            source_keyword: String::new(),
            source_page: 0,
            edit_v,
            edit_vfb,
            edit_d,
            edit_dfb,
            scroll: ScrollHandle::new(),
            tab: 0,
            add_open: false,
            new_name,
            new_version_url,
            new_download_url,
            new_bin_dir,
        }
    }

    /// 应用网络配置回填
    #[allow(clippy::too_many_arguments)]
    pub fn apply_values(
        &mut self,
        proxy: String,
        ssl_verify: bool,
        timeout: u32,
        ttl: u32,
        token: String,
        mirror: String,
        symlink_dir: String,
        window: &mut Window,
        cx: &mut Context<GuiApp>,
    ) {
        self.proxy.update(cx, |s, cx| s.set_value(&proxy, window, cx));
        self.timeout.update(cx, |s, cx| s.set_value(timeout.to_string(), window, cx));
        self.ttl.update(cx, |s, cx| s.set_value(ttl.to_string(), window, cx));
        self.token.update(cx, |s, cx| s.set_value(&token, window, cx));
        self.mirror.update(cx, |s, cx| s.set_value(&mirror, window, cx));
        self.ssl_verify = ssl_verify;
        self.symlink_dir = symlink_dir;
        self.loaded = true;
    }

    /// 应用 SDK 源回填：重建草稿列表
    pub fn apply_sources(&mut self, items: Vec<SdkSourceEntry>, window: &mut Window, cx: &mut Context<GuiApp>) {
        self.sources = items
            .into_iter()
            .map(|e| SourceDraft {
                is_builtin: find_seed(&e.name).is_some(),
                name: e.name,
                version_url: e.version_url.unwrap_or_default(),
                version_fallback_url: e.version_fallback_url.unwrap_or_default(),
                download_url: e.download_url.unwrap_or_default(),
                download_fallback_url: e.download_fallback_url.unwrap_or_default(),
            })
            .collect();
        self.source_keyword.clear();
        self.source_page = 0;
        self.source_search.update(cx, |s, cx| s.set_value("", window, cx));
    }

    /// 按关键字过滤后的源草稿（保留原索引，便于弹窗回写）
    fn filtered_sources(&self) -> Vec<(usize, &SourceDraft)> {
        let kw = self.source_keyword.to_lowercase();
        self.sources
            .iter()
            .enumerate()
            .filter(|(_, s)| kw.is_empty() || s.name.to_lowercase().contains(&kw))
            .collect()
    }
}

/// 设置页 tab（标签固定带“设置”二字，按设计图顺序）
fn tab_labels(t: &crate::i18n::Lang) -> [(&'static str, IconName); 4] {
    [
        (t.settings.tab_interface, IconName::Monitor),
        (t.settings.tab_env, IconName::Globe),
        (t.settings.tab_sources, IconName::Package),
        (t.settings.tab_custom, IconName::Puzzle),
    ]
}

/// tab 对应的分区索引（内容卡顺序：界面/网络/目录/源/自定义）
const TAB_SECTION: [usize; 4] = [0, 1, 3, 4];

/// 渲染设置页（tab 锚点栏 + 单一滚动容器堆叠全部分区）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let st = &app.settings;
    let tabs = tab_labels(t);

    let tab_bar = h_flex()
        .id("settings-tabs")
        .gap_1()
        .border_b_1()
        .border_color(theme.border)
        .children(tabs.iter().enumerate().map(|(i, (label, icon))| {
            let selected = st.tab == i;
            let target = TAB_SECTION[i];
            h_flex()
                .id(ElementId::Name(format!("settings-tab-{i}").into()))
                .gap_2()
                .items_center()
                .px_4()
                .py_2p5()
                .cursor_pointer()
                .border_b_2()
                .when(selected, |el| el.border_color(theme.primary).text_color(theme.primary))
                .when(!selected, |el| {
                    el.border_color(theme.transparent)
                        .text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.settings.tab = i;
                    app.settings.scroll.scroll_to_top_of_item(target);
                    cx.notify();
                }))
                .child(Icon::new(*icon).small())
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child((*label).to_string()))
        }));

    // 滚动容器的直接子元素 = 5 个分区卡（顺序与 TAB_SECTION 对应）
    let sections: Vec<AnyElement> = vec![
        interface_card(app, cx).into_any_element(),
        network_card(app, cx).into_any_element(),
        dirs_card(app, cx).into_any_element(),
        sources_card(app, cx).into_any_element(),
        custom_card(app, cx).into_any_element(),
    ];

    v_flex()
        .size_full()
        .gap_4()
        .child(crate::components::page_header(
            IconName::Settings,
            t.settings.title,
            t.settings.subtitle,
            cx,
        ))
        .child(tab_bar)
        .child(
            v_flex()
                .id("settings-scroll")
                .flex_1()
                .min_h_0()
                .gap_4()
                .pb_4()
                .max_w(px(1000.))
                .overflow_y_scroll()
                .track_scroll(&st.scroll)
                .children(sections)
                .vertical_scrollbar(&st.scroll),
        )
}

/// 分组卡头部（左图标 + 组名 + 描述）
fn group_head(icon: IconName, title: &'static str, desc: &'static str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    h_flex()
        .w(px(200.))
        .flex_shrink_0()
        .gap_2()
        .items_start()
        .child(
            h_flex()
                .size(px(36.))
                .rounded(px(10.))
                .justify_center()
                .bg(tks.primary_soft_bg)
                .child(Icon::new(icon).large().text_color(tks.primary_soft_fg)),
        )
        .child(
            v_flex()
                .gap_0p5()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title.to_string()),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(desc.to_string())),
        )
}

/// 设置行（label + 描述 在左，控件在右）
fn setting_row(label: &'static str, desc: &'static str, control: impl IntoElement) -> Div {
    h_flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_1p5()
        .child(
            v_flex()
                .gap_0p5()
                .min_w_0()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label.to_string()))
                .child(div().text_xs().text_color(gpui_kit::rgb(0x64748b)).child(desc.to_string())),
        )
        .child(control)
}

/// 固定宽度输入容器
fn input_w(w: f32, input: impl IntoElement) -> Div {
    div().w(px(w)).child(input)
}

/// 表单字段（小标题 + 控件）
fn form_field(label: &'static str, input: impl IntoElement, w: f32, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .gap_1()
        .w(px(w))
        .child(div().text_xs().text_color(theme.muted_foreground).child(label.to_string()))
        .child(input)
}

/// 设置分区选择 chip（选中蓝描边）
fn setting_chip<F>(
    id: &'static str,
    label: &'static str,
    selected: bool,
    icon: Option<IconName>,
    cx: &Context<GuiApp>,
    on: F,
) -> impl IntoElement
where
    F: Fn(&mut GuiApp, &mut Context<GuiApp>) + 'static,
{
    let theme = cx.theme();
    div()
        .id(ElementId::Name(format!("{id}-{label}").into()))
        .px_3()
        .py_1p5()
        .rounded(px(6.))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(selected, |el| {
            el.border_1()
                .border_color(theme.primary)
                .bg(theme.sidebar_accent)
                .text_color(theme.primary)
        })
        .when(!selected, |el| {
            el.bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .text_color(theme.foreground)
                .hover(|s| s.border_color(theme.sidebar_accent_foreground))
        })
        .on_click(cx.listener(move |app, _, _, cx| on(app, cx)))
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .when_some(icon, |el, ic| {
                    el.child(Icon::new(ic).small().text_color(if selected {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    }))
                })
                .child(label.to_string()),
        )
}

/// 分区卡外壳（左组头 + 右内容）
fn section_card(
    id: &'static str,
    icon: IconName,
    title: &'static str,
    desc: &'static str,
    content: impl IntoElement,
    cx: &Context<GuiApp>,
) -> Stateful<Div> {
    card(id, cx).flex().flex_col().p(px(card_pad())).child(
        h_flex()
            .gap_4()
            .items_start()
            .child(group_head(icon, title, desc, cx))
            .child(div().flex_1().min_w_0().child(content)),
    )
}

/// 默认页面下拉（选项 > 3 用下拉，替代 chip 组）
fn default_page_dropdown(app: &GuiApp) -> impl IntoElement {
    let t = app.lang;
    let current = match app.prefs.default_page.as_str() {
        "sdk" => t.common.nav_sdk,
        "projects" => t.common.nav_projects,
        "settings" => t.common.nav_settings,
        "diagnostics" => t.common.nav_diagnostics,
        _ => t.common.nav_overview,
    };
    let menu = DropdownButton::new("default-page-dropdown")
        .button(
            Button::new("default-page-btn")
                .outline()
                .small()
                .w(px(180.))
                .icon(IconName::House)
                .label(current),
        )
        .dropdown_menu_with_anchor(Anchor::TopRight, |menu, _, _| {
            menu.menu(t.common.nav_overview, Box::new(SetDefaultOverview))
                .menu(t.common.nav_sdk, Box::new(SetDefaultSdk))
                .menu(t.common.nav_projects, Box::new(SetDefaultProjects))
                .menu(t.common.nav_settings, Box::new(SetDefaultSettings))
                .menu(t.common.nav_diagnostics, Box::new(SetDefaultDiagnostics))
        });
    div().w(px(220.)).child(menu)
}

/// 界面偏好卡（语言/主题/默认页面/密度——gui.toml 即时生效）
fn interface_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;

    let lang_row = setting_row(
        t.settings.lbl_lang,
        t.settings.lbl_lang_desc,
        h_flex()
            .gap_2()
            .child(setting_chip(
                "lang-zh",
                "中文",
                app.prefs.lang == GuiLang::Zh,
                None,
                cx,
                |app, cx| app.set_lang(GuiLang::Zh, cx),
            ))
            .child(setting_chip(
                "lang-en",
                "English",
                app.prefs.lang == GuiLang::En,
                None,
                cx,
                |app, cx| app.set_lang(GuiLang::En, cx),
            )),
    );

    let theme_row = setting_row(
        t.settings.lbl_theme,
        t.settings.lbl_theme_desc,
        h_flex()
            .gap_2()
            .child(setting_chip(
                "theme-light",
                t.common.theme_light,
                app.prefs.theme_mode == GuiThemeMode::Light,
                Some(IconName::Sun),
                cx,
                |app, cx| app.set_theme(GuiThemeMode::Light, cx),
            ))
            .child(setting_chip(
                "theme-dark",
                t.common.theme_dark,
                app.prefs.theme_mode == GuiThemeMode::Dark,
                Some(IconName::Moon),
                cx,
                |app, cx| app.set_theme(GuiThemeMode::Dark, cx),
            ))
            .child(setting_chip(
                "theme-system",
                t.common.theme_system,
                app.prefs.theme_mode == GuiThemeMode::System,
                Some(IconName::Monitor),
                cx,
                |app, cx| app.set_theme(GuiThemeMode::System, cx),
            )),
    );

    let page_row = setting_row(
        t.settings.lbl_default_page,
        t.settings.lbl_default_page_desc,
        default_page_dropdown(app),
    );

    let density_row = setting_row(
        t.settings.lbl_density,
        t.settings.lbl_density_desc,
        h_flex()
            .gap_2()
            .child(setting_chip(
                "density-standard",
                t.settings.density_standard,
                app.prefs.density == GuiDensity::Standard,
                None,
                cx,
                |app, cx| {
                    app.prefs.density = GuiDensity::Standard;
                    set_density(GuiDensity::Standard);
                    app.prefs.save();
                    cx.notify();
                },
            ))
            .child(setting_chip(
                "density-compact",
                t.settings.density_compact,
                app.prefs.density == GuiDensity::Compact,
                None,
                cx,
                |app, cx| {
                    app.prefs.density = GuiDensity::Compact;
                    set_density(GuiDensity::Compact);
                    app.prefs.save();
                    cx.notify();
                },
            )),
    );

    section_card(
        "settings-interface",
        IconName::Monitor,
        t.settings.grp_interface,
        t.settings.grp_interface_desc,
        v_flex()
            .gap_1()
            .child(lang_row)
            .child(theme_row)
            .child(page_row)
            .child(density_row),
        cx,
    )
}

/// SDKM 环境卡（网络表单 + 可选保存）
fn network_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let st = &app.settings;

    let content = v_flex()
        .gap_1()
        .child(setting_row(
            t.settings.lbl_proxy,
            t.settings.lbl_proxy_desc,
            input_w(420., Input::new(&st.proxy)),
        ))
        .child(setting_row(
            t.settings.lbl_ssl,
            t.settings.lbl_ssl_desc,
            div()
                .id("ssl-toggle")
                .px_4()
                .py_1p5()
                .rounded_full()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(st.ssl_verify, |el| el.bg(theme.primary).text_color(rgb(0xffffff)))
                .when(!st.ssl_verify, |el| el.bg(theme.muted).text_color(theme.muted_foreground))
                .on_click(cx.listener(|app, _, _, cx| {
                    app.settings.ssl_verify = !app.settings.ssl_verify;
                    cx.notify();
                }))
                .child(
                    if st.ssl_verify {
                        t.settings.ssl_on
                    } else {
                        t.settings.ssl_off
                    }
                    .to_string(),
                ),
        ))
        .child(setting_row(
            t.settings.lbl_timeout,
            t.settings.lbl_timeout_desc,
            input_w(160., Input::new(&st.timeout).small()),
        ))
        .child(setting_row(
            t.settings.lbl_cache_ttl,
            t.settings.lbl_cache_ttl_desc,
            input_w(160., Input::new(&st.ttl).small()),
        ))
        .child(setting_row(
            t.settings.lbl_token,
            t.settings.lbl_token_desc,
            input_w(420., Input::new(&st.token)),
        ))
        .child(setting_row(
            t.settings.lbl_mirror,
            t.settings.lbl_mirror_desc,
            input_w(420., Input::new(&st.mirror)),
        ))
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .pt_2()
                .border_t_1()
                .border_color(theme.border)
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(t.settings.save_hint.to_string()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(btn(
                            "network-reload",
                            BtnVariant::Outline,
                            BtnSize::Small,
                            t.settings.btn_reload,
                            Some(IconName::RefreshCw),
                            false,
                            false,
                            cx.listener(|app, _, _, cx| {
                                app.settings.loaded = false;
                                app.dispatch(TaskId::ConfigLoad, cx);
                            }),
                            cx,
                        ))
                        .child(btn(
                            "network-save",
                            BtnVariant::Primary,
                            BtnSize::Small,
                            t.settings.btn_save,
                            Some(IconName::Save),
                            false,
                            false,
                            cx.listener(|app, _, _, cx| {
                                let proxy = app.settings.proxy.read(cx).value().trim().to_string();
                                let timeout = app.settings.timeout.read(cx).value().trim().parse::<u32>().unwrap_or(30);
                                let ttl = app.settings.ttl.read(cx).value().trim().parse::<u32>().unwrap_or(3600);
                                let token = app.settings.token.read(cx).value().trim().to_string();
                                let mirror = app.settings.mirror.read(cx).value().trim().to_string();
                                let ne = |s: String| if s.is_empty() { None } else { Some(s) };
                                app.dispatch(
                                    TaskId::ConfigSaveNetwork {
                                        proxy: ne(proxy),
                                        ssl_verify: app.settings.ssl_verify,
                                        timeout,
                                        ttl,
                                        token: ne(token),
                                        mirror: ne(mirror),
                                    },
                                    cx,
                                );
                            }),
                            cx,
                        )),
                ),
        );

    section_card(
        "settings-network",
        IconName::Globe,
        t.settings.grp_network,
        t.settings.grp_network_desc,
        content,
        cx,
    )
}

/// 目录设置卡（Home/存储只读，符号链接目录可浏览选择）
fn dirs_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let st = &app.settings;

    let path_row = |label: &'static str, desc: &'static str, path: String, action: Option<AnyElement>| -> Div {
        setting_row(
            label,
            desc,
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .max_w(px(420.))
                        .text_sm()
                        .text_color(theme.foreground)
                        .truncate()
                        .child(path),
                )
                .children(action),
        )
    };

    let open_dir = |id: &'static str, path: String, cx: &Context<GuiApp>| {
        div()
            .id(ElementId::Name(id.into()))
            .cursor_pointer()
            .text_color(tks.primary_soft_fg)
            .hover(|s| s.text_color(theme.primary))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.dispatch(TaskId::OpenFile { path: path.clone() }, cx);
            }))
            .child(Icon::new(IconName::FolderOpen).small())
            .into_any_element()
    };

    let symlink_display = if st.symlink_dir.is_empty() {
        format!("{} ({})", t.settings.symlink_follow_home, app.sys_info.home_path)
    } else {
        st.symlink_dir.clone()
    };

    let symlink_action = h_flex()
        .gap_1()
        .when(!st.symlink_dir.is_empty(), |el| {
            el.child(btn(
                "symlink-follow-home",
                BtnVariant::Ghost,
                BtnSize::Small,
                t.settings.symlink_follow_home,
                None,
                false,
                false,
                cx.listener(|app, _, _, cx| {
                    app.dispatch(TaskId::ConfigSaveSymlink { path: String::new() }, cx);
                }),
                cx,
            ))
        })
        .child(btn(
            "symlink-pick",
            BtnVariant::Outline,
            BtnSize::Small,
            t.settings.btn_browse,
            Some(IconName::FolderOpen),
            false,
            st.picking_symlink,
            cx.listener(|app, _, _, cx| {
                app.settings.picking_symlink = true;
                app.dispatch(
                    TaskId::PickSymlinkDir {
                        title: t.settings.symlink_pick_title.to_string(),
                    },
                    cx,
                );
            }),
            cx,
        ))
        .into_any_element();

    let content = v_flex()
        .gap_1()
        .child(path_row(
            t.settings.lbl_home,
            t.settings.lbl_home_desc,
            app.sys_info.home_path.clone(),
            Some(open_dir("open-home", app.sys_info.home_path.clone(), cx)),
        ))
        .child(path_row(
            t.settings.lbl_store,
            t.settings.lbl_store_desc,
            app.sys_info.store_path.clone(),
            Some(open_dir("open-store", app.sys_info.store_path.clone(), cx)),
        ))
        .child(path_row(
            t.settings.lbl_symlink,
            t.settings.lbl_symlink_desc,
            symlink_display,
            Some(symlink_action),
        ))
        .child(
            // 修改风险提示
            h_flex()
                .gap_1p5()
                .items_start()
                .pl_1()
                .child(Icon::new(IconName::CircleAlert).small().text_color(theme.warning))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.warning)
                        .child(t.settings.symlink_warning.to_string()),
                ),
        );

    section_card(
        "settings-dirs",
        IconName::FolderOpen,
        t.settings.grp_dirs_title,
        t.settings.grp_dirs_desc,
        content,
        cx,
    )
}

/// SDK 源设置卡（搜索 + 分页 + 弹窗编辑 4 个地址 + 底部统一保存）
fn sources_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let st = &app.settings;

    let filtered = st.filtered_sources();
    let total = filtered.len();
    let pages = total.div_ceil(PAGE_SIZE).max(1);
    let page = st.source_page.min(pages - 1);
    let page_items: Vec<(usize, &SourceDraft)> = filtered
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|(i, d)| (*i, *d))
        .collect();

    let show = |s: &str| -> String {
        if s.trim().is_empty() {
            "—".to_string()
        } else {
            s.to_string()
        }
    };

    let mut content = v_flex().gap_2();

    // 搜索行
    content = content.child(
        h_flex()
            .justify_between()
            .items_center()
            .gap_3()
            .child(div().w(px(220.)).child(Input::new(&st.source_search).small()))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("{} {}", total, t.settings.sources_count_suffix)),
            ),
    );

    if st.sources.is_empty() {
        content = content.child(empty_state(IconName::Package, t.common.empty_list, "", cx));
    } else if page_items.is_empty() {
        content = content.child(empty_state(IconName::Search, t.common.no_results, "", cx));
    } else {
        let rows = page_items
            .iter()
            .map(|(idx, d)| {
                let idx = *idx;
                h_flex()
                    .id(ElementId::NamedInteger("src-row".into(), idx as u64))
                    .gap_3()
                    .items_center()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .w(px(160.))
                            .flex_shrink_0()
                            .gap_2()
                            .items_center()
                            .child(sdk_icon(&d.name, 26., cx))
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(d.name.clone()))
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded(px(4.))
                                    .text_xs()
                                    .bg(if d.is_builtin { tks.primary_soft_bg } else { theme.muted })
                                    .text_color(if d.is_builtin {
                                        tks.primary_soft_fg
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(
                                        if d.is_builtin {
                                            t.settings.tag_builtin
                                        } else {
                                            t.settings.tag_custom
                                        }
                                        .to_string(),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(format!(
                                "{}: {}",
                                t.settings.lbl_version_primary,
                                show(&d.version_url)
                            )))
                            .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(format!(
                                "{}: {}",
                                t.settings.lbl_download_primary,
                                show(&d.download_url)
                            ))),
                    )
                    .child(btn(
                        ElementId::NamedInteger("src-edit".into(), idx as u64),
                        BtnVariant::Outline,
                        BtnSize::Small,
                        t.settings.btn_edit,
                        Some(IconName::Pencil),
                        false,
                        false,
                        cx.listener(move |app, _, window, cx| {
                            open_source_edit(app, window, cx, idx);
                        }),
                        cx,
                    ))
            })
            .collect::<Vec<_>>();

        content = content.child(v_flex().gap_1().children(rows)).child(pager(
            "sources-pager",
            page,
            pages,
            t.common.btn_prev,
            t.common.btn_next,
            cx,
            |app, cx| {
                app.settings.source_page = app.settings.source_page.saturating_sub(1);
                cx.notify();
            },
            |app, cx| {
                app.settings.source_page += 1;
                cx.notify();
            },
        ));
    }

    // 底部提示 + 重新加载 + 统一保存
    content = content.child(
        h_flex()
            .justify_between()
            .items_center()
            .pt_2()
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t.settings.sources_hint.to_string()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(btn(
                        "sources-reload",
                        BtnVariant::Outline,
                        BtnSize::Small,
                        t.settings.btn_reload,
                        Some(IconName::RefreshCw),
                        false,
                        false,
                        cx.listener(|app, _, _, cx| {
                            app.dispatch(TaskId::SdkSourcesLoad, cx);
                        }),
                        cx,
                    ))
                    .child(btn(
                        "sources-save",
                        BtnVariant::Primary,
                        BtnSize::Small,
                        t.settings.btn_save_sources,
                        Some(IconName::Save),
                        false,
                        false,
                        cx.listener(|app, _, _, cx| {
                            let entries = app
                                .settings
                                .sources
                                .iter()
                                .map(|d| SdkSourceEntry {
                                    name: d.name.clone(),
                                    version_url: ne_opt(d.version_url.clone()),
                                    version_fallback_url: ne_opt(d.version_fallback_url.clone()),
                                    download_url: ne_opt(d.download_url.clone()),
                                    download_fallback_url: ne_opt(d.download_fallback_url.clone()),
                                })
                                .collect();
                            app.dispatch(TaskId::SdkSourcesSaveFull { entries }, cx);
                        }),
                        cx,
                    )),
            ),
    );

    section_card(
        "settings-sources",
        IconName::Package,
        t.settings.grp_sources,
        t.settings.grp_sources_desc,
        content,
        cx,
    )
}

fn ne_opt(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.trim().to_string())
    }
}

/// 打开 SDK 源编辑弹窗（4 个地址；确定后只写草稿，底部保存统一生效）
fn open_source_edit(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>, idx: usize) {
    let t = app.lang;
    let Some(d) = app.settings.sources.get(idx).cloned() else {
        return;
    };
    app.settings.edit_v.update(cx, |s, cx| s.set_value(&d.version_url, window, cx));
    app.settings
        .edit_vfb
        .update(cx, |s, cx| s.set_value(&d.version_fallback_url, window, cx));
    app.settings.edit_d.update(cx, |s, cx| s.set_value(&d.download_url, window, cx));
    app.settings
        .edit_dfb
        .update(cx, |s, cx| s.set_value(&d.download_fallback_url, window, cx));

    let view = cx.entity();
    let content_view = view.clone();
    let ok_view = view.clone();
    let title = format!("{}: {}", t.settings.source_dialog_title, d.name);
    window.open_alert_dialog(cx, move |alert, _, _cx| {
        alert
            .width(px(660.))
            .title(title.clone())
            .content({
                let view = content_view.clone();
                move |content, _, cx| {
                    let st = &view.read(cx).settings;
                    content.child(
                        v_flex()
                            .gap_3()
                            .w_full()
                            .child(form_field(t.settings.lbl_version_primary, Input::new(&st.edit_v), 580., cx))
                            .child(form_field(t.settings.lbl_version_fallback, Input::new(&st.edit_vfb), 580., cx))
                            .child(form_field(t.settings.lbl_download_primary, Input::new(&st.edit_d), 580., cx))
                            .child(form_field(t.settings.lbl_download_fallback, Input::new(&st.edit_dfb), 580., cx))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings.source_dialog_hint.to_string()),
                            ),
                    )
                }
            })
            .confirm()
            .ok_text(t.common.btn_save)
            .cancel_text(t.common.btn_cancel)
            .on_ok({
                let view = ok_view.clone();
                move |_, _, cx| {
                    let values = {
                        let app = view.read(cx);
                        let st = &app.settings;
                        (
                            st.edit_v.read(cx).value().to_string(),
                            st.edit_vfb.read(cx).value().to_string(),
                            st.edit_d.read(cx).value().to_string(),
                            st.edit_dfb.read(cx).value().to_string(),
                        )
                    };
                    view.update(cx, |app, _cx| {
                        if let Some(d) = app.settings.sources.get_mut(idx) {
                            d.version_url = values.0;
                            d.version_fallback_url = values.1;
                            d.download_url = values.2;
                            d.download_fallback_url = values.3;
                        }
                    });
                    true
                }
            })
    });
}

/// 自定义 SDK 卡（列表 + 编辑/移除 + 新增表单）
fn custom_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let st = &app.settings;

    let customs: Vec<_> = app.sdk_rows.iter().flatten().filter(|it| !it.is_builtin).collect();

    let mut content = v_flex().gap_2().child(
        h_flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t.settings.custom_sdk_desc.to_string()),
            )
            .child(btn(
                "custom-add-toggle",
                BtnVariant::Outline,
                BtnSize::Small,
                t.settings.btn_add_custom,
                Some(IconName::Plus),
                false,
                false,
                cx.listener(|app, _, _, cx| {
                    app.settings.add_open = !app.settings.add_open;
                    cx.notify();
                }),
                cx,
            )),
    );

    // 新增表单（带必填标识）
    if st.add_open {
        content = content.child(
            v_flex()
                .gap_2()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.muted)
                .p_3()
                .child(
                    h_flex()
                        .gap_3()
                        .child(form_field(
                            t.settings.new_name_required,
                            Input::new(&st.new_name).small(),
                            140.,
                            cx,
                        ))
                        .child(form_field(
                            t.settings.new_version_url,
                            Input::new(&st.new_version_url).small(),
                            300.,
                            cx,
                        ))
                        .child(form_field(
                            t.settings.new_bin_dir,
                            Input::new(&st.new_bin_dir).small(),
                            100.,
                            cx,
                        )),
                )
                .child(form_field(
                    t.settings.new_download_url,
                    Input::new(&st.new_download_url).small(),
                    560.,
                    cx,
                ))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t.settings.required_hint.to_string()),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(btn(
                                    "custom-add-cancel",
                                    BtnVariant::Ghost,
                                    BtnSize::Small,
                                    t.common.btn_cancel,
                                    None,
                                    false,
                                    false,
                                    cx.listener(|app, _, _, cx| {
                                        app.settings.add_open = false;
                                        cx.notify();
                                    }),
                                    cx,
                                ))
                                .child(btn(
                                    "custom-add-submit",
                                    BtnVariant::Primary,
                                    BtnSize::Small,
                                    t.settings.btn_add,
                                    Some(IconName::Plus),
                                    false,
                                    false,
                                    cx.listener(|app, _, window, cx| {
                                        let name = app.settings.new_name.read(cx).value().trim().to_string();
                                        if name.is_empty() {
                                            app.notify_error(t.settings.name_required, window, cx);
                                            return;
                                        }
                                        let vurl = ne_opt(app.settings.new_version_url.read(cx).value().to_string());
                                        let durl = ne_opt(app.settings.new_download_url.read(cx).value().to_string());
                                        let bin = ne_opt(app.settings.new_bin_dir.read(cx).value().to_string());
                                        app.settings.add_open = false;
                                        app.dispatch(
                                            TaskId::ConfigAddSdk {
                                                name,
                                                version_url: vurl,
                                                download_url: durl,
                                                bin_dir: bin,
                                            },
                                            cx,
                                        );
                                    }),
                                    cx,
                                )),
                        ),
                ),
        );
    }

    // 已注册自定义 SDK 列表
    if customs.is_empty() {
        content = content.child(
            v_flex()
                .items_center()
                .py_6()
                .gap_1()
                .child(
                    Icon::new(IconName::Puzzle)
                        .with_size(px(36.))
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(t.settings.custom_empty.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(t.settings.custom_empty_desc.to_string()),
                ),
        );
    } else {
        let rows = customs
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let name = it.name.clone();
                let edit_name = name.clone();
                h_flex()
                    .id(ElementId::NamedInteger("custom-row".into(), i as u64))
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .child(sdk_icon(&it.name, 32., cx))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(it.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(crate::i18n::sdk_desc(&it.name, app.prefs.lang)),
                    )
                    .when(it.installed, |el| {
                        el.child(pill(t.overview.status_installed, tks.success_bg, tks.success_fg))
                    })
                    .child(
                        h_flex()
                            .ml_auto()
                            .gap_2()
                            .child(btn(
                                ElementId::NamedInteger("custom-edit".into(), i as u64),
                                BtnVariant::Outline,
                                BtnSize::Small,
                                t.settings.btn_edit,
                                Some(IconName::Pencil),
                                false,
                                false,
                                cx.listener(move |app, _, window, cx| {
                                    if let Some(idx) = app.settings.sources.iter().position(|d| d.name == edit_name) {
                                        open_source_edit(app, window, cx, idx);
                                    }
                                }),
                                cx,
                            ))
                            .child(btn(
                                ElementId::NamedInteger("custom-remove".into(), i as u64),
                                BtnVariant::Danger,
                                BtnSize::Small,
                                t.settings.btn_remove,
                                Some(IconName::Trash),
                                false,
                                false,
                                cx.listener(move |app, _, window, cx| {
                                    open_remove_confirm(app, window, cx, name.clone());
                                }),
                                cx,
                            )),
                    )
            })
            .collect::<Vec<_>>();
        content = content.child(v_flex().gap_1().children(rows));
    }

    section_card(
        "settings-custom",
        IconName::Puzzle,
        t.settings.tab_custom,
        t.settings.custom_sdk_desc,
        content,
        cx,
    )
}

/// 移除自定义 SDK 确认弹窗
fn open_remove_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>, name: String) {
    let t = app.lang;
    let tx = app.tx.clone();
    let title = format!("{}: {name}", t.settings.btn_remove);
    let desc = t.settings.custom_sdk_desc;
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc)
            .confirm()
            .ok_text(t.settings.btn_remove)
            .cancel_text(t.common.btn_cancel)
            .ok_variant(ButtonVariant::Danger)
            .on_ok({
                let tx = tx.clone();
                let name = name.clone();
                move |_, _, _| {
                    crate::data::spawn_task(tx.clone(), TaskId::ConfigRemoveSdk { name: name.clone() });
                    true
                }
            })
    });
}
