use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};

pub fn session_bar(count: usize, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(img("branding/logo.png").w(px(40.)).h(px(40.)))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("Copy for AI"),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!("Local · {} entries in memory", count)),
        )
}
