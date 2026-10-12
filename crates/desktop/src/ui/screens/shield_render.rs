use super::{MainTab, ShieldView};
use crate::ui::{components::editor_panel, theme::Palette};
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonCustomVariant, ButtonVariants},
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
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
            Button::new(("main-tab", index))
                .w_full()
                .h(px(44.))
                .rounded_lg()
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(rgb(if selected { 0x176C64 } else { 0x101D2C }).into())
                        .foreground(rgb(0xEAF1F6).into())
                        .hover(rgb(0x254A52).into())
                        .active(rgb(0x176C64).into()),
                )
                .label(target.label())
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
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
            MainTab::Text => self.render_workspace(cx).into_any_element(),
            MainTab::History => self.render_history(cx).into_any_element(),
            MainTab::Statistics => self.render_statistics(cx).into_any_element(),
            MainTab::Settings => self.render_settings(cx).into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .bg(palette.canvas)
            .text_color(palette.ink)
            .text_size(px(15.))
            .child(
                div()
                    .w(px(220.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .bg(rgb(0x101D2C))
                    .text_color(rgb(0xEAF1F6))
                    .child(img("branding/logo.png").w(px(44.)).h(px(44.)).mb_2())
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .child("Copy for AI"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x9CAFBD))
                            .mb_6()
                            .child("Local text protection"),
                    )
                    .children(navigation)
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x62D9BF))
                            .child("Local only"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9CAFBD))
                            .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .p_6()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(30.))
                                            .font_weight(FontWeight::SEMIBOLD)
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
                                        .items_center()
                                        .gap_2()
                                        .when(self.busy, |row| {
                                            row.child(
                                                Button::new("cancel").label("Cancel").on_click(
                                                    cx.listener(|this, _, _, cx| {
                                                        this.cancel_processing(cx)
                                                    }),
                                                ),
                                            )
                                        })
                                        .child(
                                            Button::new("process-clipboard")
                                                .label("Process to clipboard")
                                                .disabled(self.busy || self.quitting)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.process_clipboard(window, cx)
                                                })),
                                        )
                                        .child(
                                            Button::new("anonymize")
                                                .primary()
                                                .h(px(42.))
                                                .rounded_lg()
                                                .font_weight(FontWeight::SEMIBOLD)
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
                            .flex()
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
                                    .text_color(if self.error {
                                        palette.danger
                                    } else {
                                        palette.success
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
    fn render_workspace(&self, cx: &Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
        div()
            .flex()
            .gap_5()
            .w_full()
            .flex_1()
            .min_h_0()
            .child(editor_panel(
                "Input",
                "source-text",
                &self.input,
                Button::new("load")
                    .label("Load clipboard")
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.load_clipboard(window, cx))),
                palette,
            ))
            .child(editor_panel(
                "Output",
                "result-text",
                &self.output,
                Button::new("copy")
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
