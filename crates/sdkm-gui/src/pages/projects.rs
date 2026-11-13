//! 项目管理页：自动加载/目录选择 → 读取 .sdkm.toml → 版本下拉编辑 → 保存/清除（p3 设计图布局）
//!
//! 目录选择走系统 FolderBrowserDialog（后台线程 PowerShell，零依赖）；pin 编辑用
//! Select 下拉（选项 = 不固定 + 本地已装版本 + 当前 pin）。

use crate::app::GuiApp;
use crate::components::{
    BtnSize, BtnVariant, action_tile, btn, card, card_header, empty_state, kv_row, pager, pill, sdk_icon,
};
use crate::data::TaskId;
use crate::prefs::card_pad;
use gpui_kit::assets::IconName;
use gpui_kit::component::IndexPath;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 每页条数
const PAGE_SIZE: usize = 10;

/// SDK pin 下拉（SDK 名 + Select 实体）
type PinSelect = (String, Entity<SelectState<Vec<String>>>);

/// 分页后的一行（全量索引 + SDK 名 + 下拉实体）
type PinPageItem<'a> = (usize, &'a String, &'a Entity<SelectState<Vec<String>>>);

/// 项目管理页状态
pub struct ProjectsState {
    /// 各 SDK 的 pin 下拉（SDK 名 → Select）
    pub selects: Vec<PinSelect>,
    /// 已读取的项目目录
    pub dir: Option<String>,
    /// .sdkm.toml 完整路径
    pub config_path: Option<String>,
    /// 目录选择中
    pub picking: bool,
    /// 是否展示全部 SDK（默认只展示已固定的约束）
    pub show_all: bool,
    /// 构建下拉时使用的“不固定”文案（语言切换后仍按原哨兵值保存）
    pub none_label: String,
    /// 最近读取的项目 pins（SDK 列表晚到时用于重建下拉）
    pub pins: Vec<(String, String)>,
    /// 搜索框（显示全部 SDK 时过滤名称）
    pub search: Entity<InputState>,
    pub keyword: String,
    pub page: usize,
}

impl ProjectsState {
    pub fn new(window: &mut Window, cx: &mut Context<GuiApp>, lang: &'static crate::i18n::Lang) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(lang.projects.search_placeholder));
        Self {
            selects: Vec::new(),
            dir: None,
            config_path: None,
            picking: false,
            show_all: false,
            none_label: String::new(),
            pins: Vec::new(),
            search,
            keyword: String::new(),
            page: 0,
        }
    }

    /// 构建某 SDK 的下拉选项：不固定 + 本地已装版本 + 当前 pin（不在本地时插入）
    fn options_for(row: Option<&crate::data::SdkListItem>, pin: Option<&str>, none_label: &str) -> Vec<String> {
        let mut opts = vec![none_label.to_string()];
        if let Some(row) = row {
            for v in &row.versions {
                opts.push(v.clone());
            }
        }
        if let Some(p) = pin
            && !p.is_empty()
            && !opts.contains(&p.to_string())
        {
            opts.push(p.to_string());
        }
        opts
    }

    /// 应用读取结果：按 SDK 构建下拉（pin 值选中）
    #[allow(clippy::too_many_arguments)]
    pub fn apply_pins(
        &mut self,
        dir: String,
        path: String,
        pins: Vec<(String, String)>,
        rows: &[crate::data::SdkListItem],
        none_label: &str,
        window: &mut Window,
        cx: &mut Context<GuiApp>,
    ) {
        self.dir = Some(dir);
        self.none_label = none_label.to_string();
        self.pins = pins.clone();
        self.config_path = if path.is_empty() { None } else { Some(path) };
        let pin_map: std::collections::HashMap<String, String> = pins.into_iter().collect();

        // 遍历全部注册 SDK（未固定项也在列表中，显示"不固定"）
        self.selects = rows
            .iter()
            .map(|row| {
                let pin = pin_map.get(&row.name).map(|s| s.as_str());
                let options = Self::options_for(Some(row), pin, none_label);
                let selected = pin
                    .and_then(|p| options.iter().position(|o| o == p))
                    .map(|i| IndexPath::default().row(i));
                let state = cx.new(|cx| SelectState::new(options, selected, window, cx));
                (row.name.clone(), state)
            })
            .collect();
    }

    /// 用最新 SDK 列表重建下拉（项目 pins 先到、SDK 列表后到时的补建）
    pub fn rebuild(
        &mut self,
        rows: &[crate::data::SdkListItem],
        none_label: &str,
        window: &mut Window,
        cx: &mut Context<GuiApp>,
    ) {
        let (Some(dir), Some(path)) = (self.dir.clone(), self.config_path.clone()) else {
            return;
        };
        let pins = self.pins.clone();
        self.apply_pins(dir, path, pins, rows, none_label, window, cx);
    }

    /// 从下拉读全部 pins（跳过“不固定”项）
    fn read_pins(&self, cx: &App) -> Vec<(String, String)> {
        self.selects
            .iter()
            .filter_map(|(name, state)| {
                let val = state.read(cx).selected_value()?.to_string();
                if val.is_empty() || val == self.none_label {
                    None
                } else {
                    Some((name.clone(), val))
                }
            })
            .collect()
    }
}

/// 渲染项目管理页（标题区 → 目录卡全宽 → 约束卡+配置卡 → 快速操作三列）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let state = &app.projects;

    // ── 标题区 ──
    let header = crate::components::page_header(IconName::FolderOpen, t.projects.title, t.projects.subtitle, cx);

    // ── 项目目录卡（路径 + 已选择 tag + 更换目录按钮）──
    let dir_card =
        card("project-dir", cx).flex().flex_col().p(px(card_pad())).gap_3().child(
            h_flex()
                .gap_3()
                .items_center()
                .child(
                    h_flex()
                        .size(px(40.))
                        .rounded(px(10.))
                        .justify_center()
                        .bg(tks.primary_soft_bg)
                        .child(Icon::new(IconName::FolderOpen).text_color(tks.primary_soft_fg)),
                )
                .child(
                    v_flex()
                        .gap_0p5()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(t.projects.project_dir.to_string()),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div().text_sm().text_color(theme.muted_foreground).truncate().child(
                                        state.dir.clone().unwrap_or_else(|| t.projects.no_project_hint.to_string()),
                                    ),
                                )
                                .when(state.dir.is_some(), |el| {
                                    el.child(pill(t.projects.tag_selected, tks.success_bg, tks.success_fg))
                                }),
                        ),
                )
                .child(div().ml_auto().child(btn(
                    "pick-dir",
                    BtnVariant::Outline,
                    BtnSize::Medium,
                    if state.dir.is_some() {
                        t.projects.btn_change_dir
                    } else {
                        t.projects.btn_pick_dir
                    },
                    Some(IconName::FolderOpen),
                    state.picking,
                    state.picking,
                    cx.listener(|app, _, _, cx| {
                        app.projects.picking = true;
                        app.dispatch(
                            TaskId::PickDirectory {
                                title: t.projects.btn_pick_dir.to_string(),
                            },
                            cx,
                        );
                    }),
                    cx,
                ))),
        );

    // ── SDK 版本约束卡 ──
    let mut pins_card = card("project-pins", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_2()
        .flex_1()
        .min_w_0()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .pb_1()
                .child(card_header(t.projects.pins_title, cx))
                .when(state.dir.is_some(), |el| {
                    el.child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(btn(
                                "pins-show-all",
                                BtnVariant::Ghost,
                                BtnSize::Small,
                                if state.show_all {
                                    t.projects.only_pinned
                                } else {
                                    t.projects.show_all
                                },
                                Some(IconName::Eye),
                                false,
                                false,
                                cx.listener(|app, _, _, cx| {
                                    app.projects.show_all = !app.projects.show_all;
                                    app.projects.page = 0;
                                    cx.notify();
                                }),
                                cx,
                            ))
                            .child(btn(
                                "pins-reload",
                                BtnVariant::Ghost,
                                BtnSize::Small,
                                t.projects.read_toml,
                                Some(IconName::RefreshCw),
                                false,
                                false,
                                cx.listener(|app, _, _, cx| {
                                    // 已有目录则重读；否则弹目录选择
                                    if let Some(dir) = app.projects.dir.clone() {
                                        app.dispatch(TaskId::ProjectLoad { dir }, cx);
                                    } else {
                                        app.projects.picking = true;
                                        app.dispatch(
                                            TaskId::PickDirectory {
                                                title: t.projects.btn_pick_dir.to_string(),
                                            },
                                            cx,
                                        );
                                    }
                                }),
                                cx,
                            )),
                    )
                }),
        )
        // 搜索框独占一行，避免和标题/按钮挤在一起
        .when(state.dir.is_some(), |el| {
            el.child(h_flex().pb_1().child(div().w_full().child(Input::new(&state.search).small())))
        });

    match state.dir.clone() {
        None => {
            pins_card = pins_card.child(empty_state(
                IconName::FolderOpen,
                t.projects.no_config_found,
                t.projects.no_config_desc,
                cx,
            ));
        }
        Some(_) if state.selects.is_empty() => {
            pins_card = pins_card.child(empty_state(
                IconName::PackageOpen,
                t.projects.pins_empty,
                t.projects.pin_desc_unmanaged,
                cx,
            ));
        }
        Some(_) => {
            let kw = state.keyword.to_lowercase();
            let filtered: Vec<(usize, &PinSelect)> = state
                .selects
                .iter()
                .enumerate()
                .filter(|(_, (name, select))| {
                    let selected = select.read(cx).selected_value().map(|v| v.to_string()).unwrap_or_default();
                    let pinned = !selected.is_empty() && selected != state.none_label;
                    (state.show_all || pinned) && (kw.is_empty() || name.to_lowercase().contains(&kw))
                })
                .collect();
            let total = filtered.len();
            let pages = total.div_ceil(PAGE_SIZE).max(1);
            let page = state.page.min(pages - 1);

            if total == 0 {
                pins_card = pins_card.child(empty_state(
                    IconName::PackageOpen,
                    if kw.is_empty() {
                        t.projects.pins_empty
                    } else {
                        t.common.no_results
                    },
                    if kw.is_empty() { t.projects.show_all_hint } else { "" },
                    cx,
                ));
            } else {
                let mut rows = v_flex();
                let page_items: Vec<PinPageItem> = filtered
                    .iter()
                    .skip(page * PAGE_SIZE)
                    .take(PAGE_SIZE)
                    .map(|(i, (name, select))| (*i, name, select))
                    .collect();
                for (i, name, select) in page_items {
                    // 描述取当前下拉选中的 pin（全局激活版本与项目约束无关）
                    let selected = select.read(cx).selected_value().map(|v| v.to_string()).unwrap_or_default();
                    let desc = if selected.is_empty() || selected == state.none_label {
                        t.projects.desc_pin_unset.to_string()
                    } else {
                        format!("{} {selected}{}", t.projects.desc_pin_set, t.projects.desc_pin_suffix)
                    };
                    rows = rows.child(
                        h_flex()
                            .id(ElementId::NamedInteger("pin-row".into(), i as u64))
                            .items_center()
                            .gap_3()
                            .px_3()
                            .py_2p5()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(sdk_icon(name, 36., cx))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_0p5()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child(name.clone()),
                                    )
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(desc)),
                            )
                            .child(
                                div()
                                    .w(px(180.))
                                    .child(Select::new(select).placeholder(state.none_label.as_str())),
                            ),
                    );
                }
                pins_card = pins_card.child(rows);
                if pages > 1 {
                    pins_card = pins_card.child(pager(
                        "project-pager",
                        page,
                        pages,
                        t.common.btn_prev,
                        t.common.btn_next,
                        cx,
                        |app, cx| {
                            app.projects.page = app.projects.page.saturating_sub(1);
                            cx.notify();
                        },
                        |app, cx| {
                            app.projects.page += 1;
                            cx.notify();
                        },
                    ));
                }
            }
            pins_card = pins_card.child(
                // 优先级提示条（设计图蓝底提示）
                h_flex()
                    .mt_2()
                    .gap_2()
                    .items_center()
                    .rounded(px(8.))
                    .bg(tks.primary_soft_bg)
                    .px_3()
                    .py_2()
                    .child(Icon::new(IconName::Info).text_color(tks.primary_soft_fg))
                    .child(
                        div()
                            .text_sm()
                            .text_color(tks.primary_soft_fg)
                            .child(t.projects.pins_override_hint.to_string()),
                    ),
            );
        }
    }

    // ── 右列：配置文件信息卡 ──
    let info_card = card("project-info", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .w(px(340.))
        .flex_shrink_0()
        .child(card_header(t.projects.config_info, cx))
        .child(
            // 配置文件路径块（浅底 + 打开按钮）
            h_flex()
                .justify_between()
                .gap_2()
                .rounded(px(8.))
                .bg(theme.muted)
                .p_3()
                .child(
                    v_flex()
                        .flex_1()
                        .gap_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t.projects.config_path.to_string()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .truncate()
                                .child(state.config_path.clone().unwrap_or_else(|| "—".into())),
                        ),
                )
                .when_some(state.config_path.clone(), |el, path| {
                    el.child(
                        div()
                            .id("open-project-config")
                            .cursor_pointer()
                            .text_color(tks.primary_soft_fg)
                            .hover(|s| s.text_color(theme.primary))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.dispatch(TaskId::OpenFile { path: path.clone() }, cx);
                            }))
                            .child(Icon::new(IconName::ExternalLink).small()),
                    )
                }),
        )
        .child(kv_row(t.projects.config_source, t.projects.source_project_dir_current, cx));

    // ── 快速操作卡（三列 action_tile）──
    let qa_card = card("project-qa", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .child(card_header(t.overview.quick_actions, cx))
        .child(
            h_flex()
                .gap_3()
                .child(action_tile(
                    "pqa-apply",
                    IconName::RefreshCw,
                    t.projects.qa_apply,
                    t.projects.qa_apply_desc,
                    cx.listener(|app, _, _, cx| {
                        // 应用配置 = 保存当前下拉 pins
                        let Some(dir) = app.projects.dir.clone() else {
                            return;
                        };
                        let pins = app.projects.read_pins(cx);
                        app.dispatch(TaskId::ProjectSave { dir, pins }, cx);
                    }),
                    cx,
                ))
                .child(action_tile(
                    "pqa-clear",
                    IconName::Trash,
                    t.projects.qa_clear,
                    t.projects.qa_clear_desc,
                    cx.listener(|app, _, window, cx| open_clear_confirm(app, window, cx)),
                    cx,
                ))
                .child(action_tile(
                    "pqa-copy",
                    IconName::SquareTerminal,
                    t.projects.qa_copy_cmd,
                    t.projects.qa_copy_desc,
                    cx.listener(|app, _, _, cx| {
                        // 复制 sdkm use 用法示例：有 pin 用真实命令，没有则给占位示例
                        let dir = app.projects.dir.clone().unwrap_or_default();
                        let pins = app.projects.read_pins(cx);
                        let mut lines = Vec::new();
                        if !dir.is_empty() {
                            lines.push(format!("cd {dir}"));
                        }
                        if pins.is_empty() {
                            lines.push("sdkm use <SDK> <VERSION>".to_string());
                        } else {
                            lines.extend(pins.iter().map(|(n, v)| format!("sdkm use {n} {v}")));
                        }
                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(lines.join("\n")));
                    }),
                    cx,
                )),
        );

    v_flex()
        .gap_4()
        .child(header)
        .child(dir_card)
        .child(h_flex().gap_4().items_start().child(pins_card).child(info_card))
        .child(qa_card)
}

/// 清除项目配置确认弹窗（空 pins 保存）
fn open_clear_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>) {
    let Some(dir) = app.projects.dir.clone() else {
        return;
    };
    let t = app.lang;
    let tx = app.tx.clone();
    let title = t.projects.clear_confirm_title.to_string();
    let desc = t.projects.clear_confirm_desc.to_string();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc.clone())
            .confirm()
            .ok_text(t.common.btn_confirm)
            .cancel_text(t.common.btn_cancel)
            .ok_variant(ButtonVariant::Danger)
            .on_ok({
                let tx = tx.clone();
                let dir = dir.clone();
                move |_, _, _| {
                    crate::data::spawn_task(
                        tx.clone(),
                        TaskId::ProjectSave {
                            dir: dir.clone(),
                            pins: Vec::new(),
                        },
                    );
                    true
                }
            })
    });
}
