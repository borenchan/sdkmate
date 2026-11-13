//! 首次启动引导：config.toml 缺失时替代正常页面，提供一键初始化与 Windows 提权重启。

use crate::app::GuiApp;
use crate::components::{BtnSize, BtnVariant, btn};
use crate::data::TaskId;
use crate::platform;
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 渲染首次启动引导页（保留标题栏，隐藏侧边栏）
pub fn render(app: &GuiApp, cx: &Context<GuiApp>) -> impl IntoElement {
    let t = app.lang;
    let theme = cx.theme();
    let tks = crate::theme::tokens(cx);
    let admin_needed = cfg!(windows) && !platform::is_elevated();
    let steps = [
        t.onboarding.step_store,
        t.onboarding.step_path,
        t.onboarding.step_links,
        t.onboarding.step_hook,
    ];
    let restart_failed = t.onboarding.admin_restart_failed.to_string();

    v_flex()
        .flex_1()
        .min_h_0()
        .h_full()
        .justify_center()
        .items_center()
        .p_8()
        .child(
            v_flex()
                .w(px(600.))
                .max_w_full()
                .gap_4()
                .p_6()
                .rounded(px(14.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            h_flex()
                                .size(px(44.))
                                .rounded(px(12.))
                                .justify_center()
                                .bg(tks.primary_soft_bg)
                                .child(Icon::new(IconName::Package).text_color(theme.primary)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(t.onboarding.title))
                                .child(div().text_sm().text_color(theme.muted_foreground).child(t.onboarding.subtitle)),
                        ),
                )
                .child(v_flex().gap_2().children(steps.iter().map(|step| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::CircleCheck).small().text_color(tks.success_fg))
                        .child(div().text_sm().text_color(theme.foreground).child(*step))
                })))
                .when(admin_needed, |el| {
                    el.child(
                        div()
                            .p_3()
                            .rounded(px(8.))
                            .min_w_0()
                            .overflow_hidden()
                            .bg(tks.danger_bg)
                            .text_sm()
                            .text_color(tks.danger_fg)
                            .child(t.onboarding.admin_warning),
                    )
                })
                .when(app.initializing, |el| {
                    el.child(
                        h_flex().gap_2().items_center().child(Spinner::new().small()).child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(t.onboarding.initializing),
                        ),
                    )
                })
                .when_some(app.init_error.clone(), |el, err| {
                    el.child(
                        div()
                            .p_3()
                            .rounded(px(8.))
                            .min_w_0()
                            .overflow_hidden()
                            .bg(tks.danger_bg)
                            .text_sm()
                            .text_color(tks.danger_fg)
                            .child(err),
                    )
                })
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(btn(
                            "onboard-init",
                            BtnVariant::Primary,
                            BtnSize::Large,
                            t.onboarding.btn_init,
                            Some(IconName::Download),
                            app.initializing,
                            app.initializing,
                            cx.listener(|app, _, _, cx| start_init(app, cx)),
                            cx,
                        ))
                        .when(admin_needed, |el| {
                            el.child(btn(
                                "onboard-elevate",
                                BtnVariant::Outline,
                                BtnSize::Large,
                                t.onboarding.btn_elevate,
                                Some(IconName::ShieldCheck),
                                app.initializing,
                                false,
                                cx.listener(move |app, _, window, cx| {
                                    if platform::relaunch_as_admin() {
                                        cx.quit();
                                    } else {
                                        app.notify_error(restart_failed.clone(), window, cx);
                                    }
                                }),
                                cx,
                            ))
                        })
                        .child(btn(
                            "onboard-exit",
                            BtnVariant::Outline,
                            BtnSize::Large,
                            t.onboarding.btn_exit,
                            None,
                            false,
                            false,
                            cx.listener(|_, _, _, cx| cx.quit()),
                            cx,
                        )),
                ),
        )
}

/// 开始首次初始化（引导页按钮）
fn start_init(app: &mut GuiApp, cx: &mut Context<GuiApp>) {
    if app.initializing {
        return;
    }
    app.initializing = true;
    app.init_error = None;
    app.dispatch(TaskId::InitRun, cx);
    cx.notify();
}
