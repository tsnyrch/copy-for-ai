use crate::ui::theme::Palette;
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::{prelude::*, *};

pub fn editor_panel(
    label: &'static str,
    id: &'static str,
    state: &Entity<TextareaState>,
    action: impl IntoElement,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.surface)
        .p_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .child(action),
        )
        .child(
            div()
                .id(id)
                .debug_selector(|| id.into())
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(Textarea::new(state).aria_label(label).h_full().w_full()),
        )
}
