//! 诊断与维护页：真实环境检测（PATH/CLI/配置/存储/链接）+ Shell hook 状态 + 更新 + 高级操作。
//!
//! 数据全部来自 core 的结构化 API（`doctor::collect_environment_status` /
//! `shell::check_shell_hook`），不展示重复的 doctor 原始文本。

use crate::app::GuiApp;
use crate::components::{BtnSize, BtnVariant, action_tile, action_tile_danger, btn, card, pill, pill_dot};
use crate::data::{ShellStatus, TaskId};
use crate::prefs::card_pad;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 诊断页状态
pub struct DiagnosticsState {
    /// 环境健康检查（PATH / CLI / 配置 / 存储 / 链接）
    pub env: Option<sdkm_core::doctor::EnvironmentStatus>,
    /// Shell hook 状态
    pub shell: Option<ShellStatus>,
    pub checking: bool,
    pub shell_checking: bool,
    /// 环境修复（init --非 force）进行中
    pub repairing: bool,
    pub injecting: bool,
    /// Shell 详情（profile 列表）展开
    pub details_open: bool,
    pub updating: bool,
    /// 最近一次检查到的最新版本号
    pub latest: Option<String>,
}

impl DiagnosticsState {
    pub fn new() -> Self {
        Self {
            env: None,
            shell: None,
            checking: false,
            shell_checking: false,
            repairing: false,
            injecting: false,
            details_open: false,
            updating: false,
            latest: None,
        }
    }

    /// 环境是否全部正常（未检测时视为未知，返回 false 由卡片显示"—"）
    pub fn env_ok(&self) -> bool {
        self.env
            .as_ref()
            .map(|e| e.exe_in_path && e.cli_ok && e.config_ok && e.store_ok && e.links_ok)
            .unwrap_or(false)
    }

    /// hook 是否全部注册
    pub fn hook_ok(&self) -> bool {
        self.shell.as_ref().map(|s| s.all_registered).unwrap_or(false)
    }
}

/// 渲染诊断与维护页（标题区 → 2×2 卡 → 常见问题横幅）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;

    v_flex()
        .gap_4()
        .child(crate::components::page_header(
            IconName::ShieldCheck,
            t.diagnostics.title,
            t.diagnostics.subtitle,
            cx,
        ))
        .child(
            h_flex()
                .gap_4()
                .items_start()
                .child(env_card(app, cx))
                .child(shell_card(app, cx)),
        )
        .child(
            h_flex()
                .gap_4()
                .items_start()
                .child(update_card(app, cx))
                .child(advanced_card(app, cx)),
        )
        .child(faq_card(app, cx))
}

/// 卡片标题行（图标 + 标题 + 可选右侧元素）
fn card_title(icon: IconName, title: &str, right: Option<impl IntoElement>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    h_flex()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).text_color(tks.primary_soft_fg))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title.to_string()),
                ),
        )
        .children(right)
}

/// 状态行（图标 + 标签 + 值 + 绿勾/红叉）
fn status_line(icon: IconName, label: &str, value: String, ok: Option<bool>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .items_center()
        .gap_2()
        .py_1()
        .child(Icon::new(icon).small().text_color(theme.muted_foreground))
        .child(
            div()
                .w(px(104.))
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(label.to_string()),
        )
        .child(
            h_flex()
                .flex_1()
                .gap_1p5()
                .items_center()
                .min_w_0()
                .overflow_hidden()
                .child(div().text_sm().min_w_0().text_color(theme.foreground).truncate().child(value))
                .when_some(ok, |el, good| {
                    el.child(
                        Icon::new(if good {
                            IconName::CircleCheck
                        } else {
                            IconName::CircleAlert
                        })
                        .small()
                        .flex_shrink_0()
                        .text_color(if good { theme.success } else { theme.danger }),
                    )
                }),
        )
}

/// 环境诊断卡（sdkm 是否在 PATH / CLI 可执行 / 配置与目录状态 / 系统信息）
fn env_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let state = &app.diagnostics;
    let env = state.env.as_ref();

    let (ok_bg, ok_fg) = (tks.success_bg, tks.danger_bg);
    let (fg_ok, fg_bad) = (tks.success_fg, tks.danger_fg);

    let mut content = v_flex().gap_1();
    if let Some(e) = env {
        content = content
            .child(status_line(
                IconName::Package,
                t.diagnostics.row_sdkm_path,
                if e.exe_in_path {
                    t.diagnostics.value_yes.to_string()
                } else {
                    t.diagnostics.value_no.to_string()
                },
                Some(e.exe_in_path),
                cx,
            ))
            .child(status_line(
                IconName::SquareTerminal,
                t.diagnostics.row_cli,
                if e.cli_ok {
                    format!(
                        "{} (v{})",
                        t.diagnostics.value_runnable,
                        e.cli_version.as_deref().unwrap_or("?")
                    )
                } else {
                    t.diagnostics.value_not_runnable.to_string()
                },
                Some(e.cli_ok),
                cx,
            ))
            .child(status_line(
                IconName::FileText,
                t.diagnostics.row_config,
                if e.config_path.is_empty() {
                    "—".to_string()
                } else {
                    e.config_path.clone()
                },
                Some(e.config_ok),
                cx,
            ))
            .child(status_line(
                IconName::HardDrive,
                t.diagnostics.row_store,
                if e.store_path.is_empty() {
                    "—".to_string()
                } else {
                    e.store_path.clone()
                },
                Some(e.store_ok),
                cx,
            ))
            .child(status_line(
                IconName::FolderOpen,
                t.diagnostics.row_links,
                e.links_dir.clone(),
                Some(e.links_ok),
                cx,
            ))
            .child(status_line(
                IconName::Monitor,
                t.diagnostics.row_os,
                app.sys_info.os.clone(),
                None,
                cx,
            ));
    } else {
        content = content.child(
            div()
                .py_4()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(t.diagnostics.detecting.to_string()),
        );
    }

    card("diag-env", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_2()
        .flex_1()
        .min_w_0()
        .child(card_title(
            IconName::Activity,
            t.diagnostics.card_diagnose,
            env.map(|_| {
                pill_dot(
                    if state.env_ok() {
                        t.diagnostics.tag_env_ok
                    } else {
                        t.diagnostics.tag_env_warn
                    },
                    if state.env_ok() { theme.success } else { theme.warning },
                    if state.env_ok() { ok_bg } else { ok_fg },
                    if state.env_ok() { fg_ok } else { fg_bad },
                )
            }),
            cx,
        ))
        .child(content)
        .when(state.env.as_ref().is_some_and(|_| !state.env_ok()), |el| {
            el.child(btn(
                "diag-repair",
                BtnVariant::Primary,
                BtnSize::Medium,
                t.diagnostics.btn_repair,
                Some(IconName::Wrench),
                state.checking || state.repairing,
                state.repairing,
                cx.listener(|app, _, _, cx| {
                    if app.diagnostics.repairing || app.diagnostics.checking {
                        return;
                    }
                    app.diagnostics.repairing = true;
                    app.dispatch(TaskId::InitRun, cx);
                    cx.notify();
                }),
                cx,
            ))
        })
        .child(
            // 设计图：整宽浅蓝描边按钮
            div()
                .id("diag-recheck")
                .mt_1()
                .w_full()
                .h(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.primary)
                .bg(tks.primary_soft_bg)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.primary)
                .cursor_pointer()
                .hover(|s| s.bg(theme.sidebar_accent))
                .on_click(cx.listener(|app, _, _, cx| {
                    if app.diagnostics.checking {
                        return;
                    }
                    app.diagnostics.checking = true;
                    app.diagnostics.shell_checking = true;
                    app.dispatch(TaskId::DiagEnv, cx);
                    app.dispatch(TaskId::ShellStatus, cx);
                }))
                .when(state.checking, |el| el.child(Spinner::new().small()))
                .when(!state.checking, |el| el.child(Icon::new(IconName::RefreshCw).small()))
                .child(t.diagnostics.btn_recheck.to_string()),
        )
}

/// Shell 集成卡（Shell 类型 + hook 注册状态 + 注入 + profile 详情）
fn shell_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let state = &app.diagnostics;
    let hook_ok = state.hook_ok();

    let mut card_el = card("diag-shell", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_3()
        .flex_1()
        .min_w_0()
        .child(card_title(IconName::SquareTerminal, t.diagnostics.card_shell, None::<Div>, cx));

    if let Some(shell) = &state.shell {
        card_el =
            card_el
                .child(status_line(
                    IconName::SquareTerminal,
                    t.diagnostics.row_shell,
                    shell.shell.clone(),
                    None,
                    cx,
                ))
                .child(
                    // 状态横幅（与 hook 状态一致，不重复展示）
                    h_flex()
                        .gap_2p5()
                        .items_center()
                        .rounded(px(8.))
                        .bg(if hook_ok { tks.success_bg } else { tks.danger_bg })
                        .p_3()
                        .child(
                            h_flex()
                                .size(px(28.))
                                .rounded_full()
                                .justify_center()
                                .bg(if hook_ok { theme.success } else { theme.danger })
                                .text_color(rgb(0xffffff))
                                .child(
                                    Icon::new(if hook_ok {
                                        IconName::CircleCheck
                                    } else {
                                        IconName::CircleAlert
                                    })
                                    .small(),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if hook_ok { tks.success_fg } else { tks.danger_fg })
                                        .child(if hook_ok {
                                            t.diagnostics.hook_ok_title
                                        } else {
                                            t.diagnostics.hook_missing_title
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if hook_ok { tks.success_fg } else { tks.danger_fg })
                                        .child(if hook_ok {
                                            t.diagnostics.hook_ok_desc
                                        } else {
                                            t.diagnostics.hook_missing_desc
                                        }),
                                ),
                        ),
                )
                .child(
                    // 详细信息折叠行
                    h_flex()
                        .id("shell-details-toggle")
                        .items_center()
                        .justify_between()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.muted))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.diagnostics.details_open = !app.diagnostics.details_open;
                            cx.notify();
                        }))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    Icon::new(if state.details_open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .small(),
                                )
                                .child(div().text_sm().text_color(theme.foreground).child(t.diagnostics.shell_details)),
                        )
                        .child(Icon::new(IconName::ChevronRight).small().text_color(theme.muted_foreground)),
                )
                .when(state.details_open, |el| {
                    el.child(v_flex().gap_1().rounded(px(8.)).bg(theme.muted).p_3().children(
                        shell.profiles.iter().map(|(path, registered)| {
                            h_flex()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(path.clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(if *registered { theme.success } else { theme.danger })
                                        .child(if *registered {
                                            t.diagnostics.value_registered
                                        } else {
                                            t.diagnostics.value_not_registered
                                        }),
                                )
                        }),
                    ))
                })
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .when(!hook_ok, |el| {
                            el.child(btn(
                                "shell-inject",
                                BtnVariant::Primary,
                                BtnSize::Small,
                                t.diagnostics.btn_inject_hook,
                                Some(IconName::Plus),
                                state.injecting,
                                state.injecting,
                                cx.listener(|app, _, window, cx| {
                                    open_inject_confirm(app, window, cx);
                                }),
                                cx,
                            ))
                        })
                        .when(hook_ok, |el| el.child(div()))
                        .child(btn(
                            "shell-guide",
                            BtnVariant::Outline,
                            BtnSize::Small,
                            t.diagnostics.btn_guide,
                            Some(IconName::FileText),
                            false,
                            false,
                            cx.listener(|_, _, _, cx| {
                                cx.open_url("https://github.com/borenchan/sdkmate#readme");
                            }),
                            cx,
                        )),
                );
    } else {
        card_el = card_el.child(
            h_flex().gap_2().items_center().py_4().child(Spinner::new().small()).child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(t.diagnostics.detecting.to_string()),
            ),
        );
    }

    card_el
}

/// sdkm 更新卡（CLI/GUI 版本 + 最新版本 + 状态 pill + 检查更新/立即更新/回退）
fn update_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let state = &app.diagnostics;

    // CLI 版本 = 同目录 sdkm 二进制的实际版本；GUI 版本 = 当前运行进程版本
    let cli_ver = state
        .env
        .as_ref()
        .and_then(|e| e.sibling_cli_version.clone())
        .or_else(|| state.env.as_ref().and_then(|e| e.cli_version.clone()));
    let gui_ver = Some(app.sys_info.version.clone());
    let latest = state.latest.clone();

    let mismatch = matches!((&cli_ver, &gui_ver), (Some(c), Some(g)) if c != g);
    let latest_newer = latest.as_deref().is_some_and(|v| {
        cli_ver.as_deref().is_some_and(|c| sdkm_core::self_update::is_newer(v, c))
            || gui_ver.as_deref().is_some_and(|g| sdkm_core::self_update::is_newer(v, g))
    });
    let need_update = mismatch || latest_newer;

    let ver_row = |label: &'static str, value: String, right: Option<AnyElement>| -> Div {
        h_flex()
            .items_center()
            .justify_between()
            .py_1()
            .child(div().text_sm().text_color(theme.muted_foreground).child(label.to_string()))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(value),
                    )
                    .children(right),
            )
    };

    let status_pill = latest.as_ref().map(|_| {
        if mismatch {
            pill(t.diagnostics.tag_version_mismatch, tks.danger_bg, tks.danger_fg).into_any_element()
        } else if need_update {
            pill(t.diagnostics.tag_new_avail, tks.primary_soft_bg, tks.primary_soft_fg).into_any_element()
        } else {
            pill(t.diagnostics.tag_up_to_date, tks.success_bg, tks.success_fg).into_any_element()
        }
    });

    let mut buttons = h_flex().gap_2();
    if need_update {
        buttons = buttons.child(btn(
            "update-run",
            BtnVariant::Primary,
            BtnSize::Medium,
            t.diagnostics.btn_update_now,
            Some(IconName::Download),
            state.updating,
            state.updating,
            cx.listener(|app, _, window, cx| {
                app.diagnostics.updating = true;
                app.open_progress_dialog(window, cx);
                app.dispatch(TaskId::UpdateRun, cx);
            }),
            cx,
        ));
    }
    buttons = buttons
        .child(btn(
            "update-check",
            if need_update {
                BtnVariant::Outline
            } else {
                BtnVariant::Primary
            },
            BtnSize::Medium,
            t.diagnostics.btn_check_update,
            Some(IconName::RefreshCw),
            state.updating,
            state.updating,
            cx.listener(|app, _, _, cx| {
                app.diagnostics.updating = true;
                app.dispatch(TaskId::UpdateCheck, cx);
            }),
            cx,
        ))
        .child(btn(
            "update-rollback",
            BtnVariant::Outline,
            BtnSize::Medium,
            t.diagnostics.btn_rollback,
            Some(IconName::Undo),
            state.updating,
            state.updating,
            cx.listener(|app, _, _, cx| {
                // 先查询回退目标版本，确认弹窗里展示具体版本号
                app.diagnostics.updating = true;
                app.dispatch(TaskId::UpdatePreviousVersion, cx);
            }),
            cx,
        ));

    card("diag-update", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_2()
        .flex_1()
        .min_w_0()
        .child(card_title(IconName::RefreshCw, t.diagnostics.card_update, None::<Div>, cx))
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(t.diagnostics.update_desc.to_string()),
        )
        .child(ver_row(
            t.diagnostics.lbl_cli_ver,
            cli_ver.as_ref().map(|v| format!("v{v}")).unwrap_or_else(|| "—".into()),
            None,
        ))
        .child(ver_row(
            t.diagnostics.lbl_gui_ver,
            gui_ver.as_ref().map(|v| format!("v{v}")).unwrap_or_else(|| "—".into()),
            None,
        ))
        .child(ver_row(
            t.diagnostics.lbl_latest_ver,
            latest.as_ref().map(|v| format!("v{v}")).unwrap_or_else(|| "—".into()),
            status_pill,
        ))
        .when(mismatch, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(tks.danger_fg)
                    .child(t.diagnostics.version_mismatch_desc.to_string()),
            )
        })
        .child(buttons)
}

/// 高级操作卡（重新初始化 / 查看配置 / 卸载；卸载为红色高危）
fn advanced_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();

    card("diag-advanced", cx)
        .flex()
        .flex_col()
        .p(px(card_pad()))
        .gap_2()
        .flex_1()
        .min_w_0()
        .child(card_title(IconName::Zap, t.diagnostics.card_advanced, None::<Div>, cx))
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(t.diagnostics.advanced_desc.to_string()),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    // 固定最小宽度 + flex_wrap：窄窗口折行，避免第三张卡溢出父容器
                    div().w(px(136.)).child(action_tile(
                        "adv-reinit",
                        IconName::RefreshCw,
                        t.diagnostics.adv_reinit,
                        t.diagnostics.adv_reinit_desc,
                        cx.listener(|app, _, window, cx| {
                            open_reinit_confirm(app, window, cx);
                        }),
                        cx,
                    )),
                )
                .child(div().w(px(136.)).child(action_tile(
                    "adv-open-config",
                    IconName::FileText,
                    t.diagnostics.adv_open_config,
                    t.diagnostics.adv_open_config_desc,
                    cx.listener(|app, _, _, cx| {
                        app.dispatch(TaskId::OpenConfigPath, cx);
                    }),
                    cx,
                )))
                .child(div().w(px(136.)).child(action_tile_danger(
                    "adv-uninstall",
                    IconName::Trash,
                    t.diagnostics.adv_uninstall,
                    t.diagnostics.adv_uninstall_desc,
                    cx.listener(|app, _, window, cx| {
                        open_uninstall_self_confirm(app, window, cx);
                    }),
                    cx,
                ))),
        )
}

/// 常见问题横幅卡
fn faq_card(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);

    card("diag-faq", cx)
        .flex()
        .items_center()
        .gap_3()
        .p(px(card_pad()))
        .child(
            h_flex()
                .size(px(32.))
                .rounded_full()
                .justify_center()
                .bg(tks.primary_soft_bg)
                .child(Icon::new(IconName::Info).text_color(tks.primary_soft_fg)),
        )
        .child(
            v_flex()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(t.diagnostics.faq.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(t.diagnostics.faq_desc.to_string()),
                ),
        )
        .child(
            h_flex()
                .gap_4()
                .ml_4()
                .child(faq_link(
                    "faq-docs",
                    t.diagnostics.link_docs,
                    "https://github.com/borenchan/sdkmate/blob/master/docs/usage.md",
                    cx,
                ))
                .child(faq_link(
                    "faq-issues",
                    t.diagnostics.link_faq,
                    "https://github.com/borenchan/sdkmate/issues",
                    cx,
                ))
                .child(faq_link(
                    "faq-github",
                    t.diagnostics.link_github,
                    "https://github.com/borenchan/sdkmate",
                    cx,
                )),
        )
}

/// 常见问题链接（点击打开浏览器）
fn faq_link(id: &'static str, label: &'static str, url: &'static str, cx: &Context<GuiApp>) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .id(ElementId::Name(id.into()))
        .gap_1p5()
        .items_center()
        .cursor_pointer()
        .text_sm()
        .text_color(theme.link)
        .hover(|s| s.text_color(theme.foreground))
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.open_url(url);
        }))
        .child(Icon::new(IconName::ExternalLink).small())
        .child(label.to_string())
}

/// 注入 hook 确认弹窗
fn open_inject_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>) {
    let t = app.lang;
    let tx = app.tx.clone();
    let title = t.diagnostics.inject_confirm_title.to_string();
    let desc = t.diagnostics.inject_confirm_desc.to_string();
    let ok_text = t.diagnostics.btn_inject_hook.to_string();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc.clone())
            .confirm()
            .ok_text(ok_text.clone())
            .cancel_text(t.common.btn_cancel)
            .on_ok({
                let tx = tx.clone();
                move |_, _, _| {
                    crate::data::spawn_task(tx.clone(), TaskId::HookInject);
                    true
                }
            })
    });
}

/// 回滚确认弹窗
pub(crate) fn open_rollback_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>, target: &str) {
    let t = app.lang;
    let view = cx.entity();
    let title = format!("{} · v{}", t.diagnostics.btn_rollback, target);
    let desc = t.diagnostics.rollback_confirm_desc.to_string();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc.clone())
            .confirm()
            .ok_text(t.common.btn_confirm)
            .cancel_text(t.common.btn_cancel)
            .ok_variant(ButtonVariant::Danger)
            .on_ok({
                let view = view.clone();
                move |_, window, cx| {
                    view.update(cx, |app, cx| {
                        // 确认后打开进度弹窗，再走与升级相同的下载/替换流程
                        app.diagnostics.updating = true;
                        app.open_progress_dialog(window, cx);
                        app.dispatch(TaskId::UpdateRollback, cx);
                    });
                    true
                }
            })
    });
}

/// 重新初始化确认弹窗
fn open_reinit_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>) {
    let t = app.lang;
    let tx = app.tx.clone();
    let title = t.diagnostics.reinit_confirm_title.to_string();
    let desc = t.diagnostics.reinit_confirm_desc.to_string();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc.clone())
            .confirm()
            .ok_text(t.common.btn_confirm)
            .cancel_text(t.common.btn_cancel)
            .on_ok({
                let tx = tx.clone();
                move |_, _, _| {
                    crate::data::spawn_task(tx.clone(), TaskId::Reinit);
                    true
                }
            })
    });
}

/// 卸载 sdkm 确认弹窗
fn open_uninstall_self_confirm(app: &mut GuiApp, window: &mut Window, cx: &mut Context<GuiApp>) {
    let t = app.lang;
    let tx = app.tx.clone();
    let title = t.diagnostics.uninstall_self_confirm_title.to_string();
    let desc = t.diagnostics.uninstall_self_confirm_desc.to_string();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(desc.clone())
            .confirm()
            .ok_text(t.diagnostics.adv_uninstall)
            .cancel_text(t.common.btn_cancel)
            .ok_variant(ButtonVariant::Danger)
            .on_ok({
                let tx = tx.clone();
                move |_, _, _| {
                    crate::data::spawn_task(tx.clone(), TaskId::UninstallSelf);
                    true
                }
            })
    });
}
