use super::action_button;
use copy_for_ai_core::EntityKind;
use gpui_kit::component::{
    ActiveTheme, Disableable,
    input::{Input, InputState},
};
use gpui_kit::{prelude::*, *};

pub fn rule_editor(
    input: &Entity<InputState>,
    kind: EntityKind,
    busy: bool,
    rows: impl IntoIterator<Item = impl IntoElement>,
    on_cycle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_add: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child("Custom sensitive terms"),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .items_center()
                .child(
                    div().flex_1().min_w(px(200.)).child(
                        Input::new(input)
                            .aria_label("Custom sensitive term")
                            .h(px(36.)),
                    ),
                )
                .child(
                    action_button("kind")
                        .label(kind.tag())
                        .disabled(busy)
                        .on_click(on_cycle),
                )
                .child(
                    action_button("add-rule")
                        .label("Add")
                        .disabled(busy)
                        .on_click(on_add),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Exact, case-sensitive whole-word matches. Terms stay in memory."),
        )
        .child(
            div()
                .id("rules-list")
                .max_h(px(85.))
                .overflow_y_scroll()
                .children(rows),
        )
}
