use super::{MainTab, ShieldView};
use crate::ui::{
    components::{action_button, editor_panel},
    theme::Palette,
};
use gpui_kit::{
    component::{
        Disableable,
        button::{ButtonCustomVariant, ButtonVariants},
    },
    prelude::*,
    *,
};

impl MainTab {
    fn label(self) -> &'static str {
        match self {
            Self::Text => "Workspace",
            Self::History => "History",
            Self::Statistics => "Statistics",
            Self::Settings => "Settings",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Text => "Text ready for AI",
            Self::History => "Recent operations, without stored documents",
            Self::Statistics => "Lifetime usage overview",
            Self::Settings => "Appearance, behavior and app information",
        }
    }
}

impl Render for ShieldView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
        let compact = window.viewport_size().width < px(960.);
        let navigation = [
            MainTab::Text,
            MainTab::History,
            MainTab::Statistics,
            MainTab::Settings,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, target)| {
            let selected = target == self.tab;
            action_button(("main-tab", index))
                .debug_selector(move || format!("main-{index}"))
                .when(!compact, |button| button.w_full())
                .when(compact, |button| button.flex_1())
                .accessibility_label(if selected {
                    format!("{} · current page", target.label())
                } else {
                    target.label().to_owned()
                })
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(if selected {
                            palette.accent_wash
                        } else {
                            palette.surface
                        })
                        .foreground(if selected {
                            palette.accent_ink
                        } else {
                            palette.muted
                        })
                        .hover(if selected {
                            palette.accent_wash
                        } else {
                            palette.hover
                        })
                        .active(palette.accent_wash),
                )
                .when(selected, |button| {
                    button.border_l_2().border_color(palette.primary)
                })
                .label(target.label())
                .font_weight(if selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = target;
                    if target == MainTab::Settings {
                        this.open_settings(cx);
                    }
                    cx.notify();
                }))
        })
        .collect::<Vec<_>>();
        let content = match self.tab {
            MainTab::Text => self.render_workspace(compact, cx).into_any_element(),
            MainTab::History => self.render_history(cx).into_any_element(),
            MainTab::Statistics => self.render_statistics(compact, cx).into_any_element(),
            MainTab::Settings => self.render_settings(cx).into_any_element(),
        };
        div()
            .tab_group()
            .size_full()
            .flex()
            .when(compact, |shell| shell.flex_col())
            .bg(palette.canvas)
            .text_color(palette.ink)
            .text_size(px(14.))
            .line_height(px(22.))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .when(!compact, |sidebar| {
                        sidebar.w(px(240.)).flex_col().border_r_1()
                    })
                    .when(compact, |sidebar| {
                        sidebar.w_full().flex_wrap().items_center().border_b_1()
                    })
                    .gap_3()
                    .p_4()
                    .bg(palette.surface)
                    .border_color(palette.border)
                    .child(img("branding/logo.png").w(px(32.)).h(px(32.)))
                    .child(
                        div()
                            .when(compact, |title| title.w_full().flex_1())
                            .text_size(px(18.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Copy for AI"),
                    )
                    .when(!compact, |sidebar| {
                        sidebar.child(
                            div()
                                .text_sm()
                                .text_color(palette.muted)
                                .mb_6()
                                .child("Local text protection"),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .w_full()
                            .when(!compact, |nav| nav.flex_col())
                            .children(navigation),
                    )
                    .when(!compact, |sidebar| {
                        sidebar
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Local only"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.muted)
                                    .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                            )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .when(compact, |content| content.w_full())
                    .max_w(px(1440.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .px(px(if compact { 20. } else { 36. }))
                    .py_6()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .when(compact, |header| header.flex_col().items_start())
                            .justify_between()
                            .gap_4()
                            .pb_4()
                            .border_b_1()
                            .border_color(palette.border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .line_height(px(24.))
                                            .font_weight(FontWeight::NORMAL)
                                            .child(self.tab.label()),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(palette.muted)
                                            .child(self.tab.description()),
                                    ),
                            )
                            .when(self.tab == MainTab::Text, |header| {
                                header.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_2()
                                        .when(self.busy, |row| {
                                            row.child(
                                                action_button("cancel")
                                                    .debug_selector(|| "cancel".into())
                                                    .label("Cancel")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.cancel_processing(cx)
                                                    })),
                                            )
                                        })
                                        .child(
                                            action_button("process-clipboard")
                                                .debug_selector(|| "process-clipboard".into())
                                                .label("Process to clipboard")
                                                .disabled(self.busy || self.quitting)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.process_clipboard(window, cx)
                                                })),
                                        )
                                        .child(
                                            action_button("anonymize")
                                                .debug_selector(|| "process-input".into())
                                                .primary()
                                                .label(if self.busy {
                                                    "Processing…"
                                                } else {
                                                    "Process input"
                                                })
                                                .disabled(self.busy || self.quitting)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.process_input(window, cx)
                                                })),
                                        ),
                                )
                            }),
                    )
                    .child(content)
                    .child(
                        div()
                            .debug_selector(|| "status-bar".into())
                            .flex()
                            .flex_shrink_0()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .pt_3()
                            .border_t_1()
                            .border_color(palette.border)
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .min_w(px(220.))
                                    .text_color(if self.error {
                                        palette.danger
                                    } else {
                                        palette.muted
                                    })
                                    .child(self.status.clone()),
                            )
                            .when(self.tab == MainTab::Text, |row| {
                                row.child(
                                    div()
                                        .text_xs()
                                        .text_color(palette.muted)
                                        .child("Review the output before sharing."),
                                )
                            }),
                    ),
            )
    }
}

impl ShieldView {
    fn render_workspace(&self, compact: bool, cx: &Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
        div()
            .flex()
            .when(compact, |editors| editors.flex_col())
            .gap_6()
            .w_full()
            .flex_1()
            .min_h_0()
            .child(editor_panel(
                "Input",
                "source-text",
                &self.input,
                action_button("load")
                    .label("Load clipboard")
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.load_clipboard(window, cx))),
                palette,
            ))
            .child(editor_panel(
                "Output",
                "result-text",
                &self.output,
                action_button("copy")
                    .label("Copy")
                    .disabled(
                        self.busy
                            || self.output_direction.is_none()
                            || self.output_direction
                                == Some(copy_for_ai_core::Direction::Unresolved),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.copy_output(cx))),
                palette,
            ))
    }
}
