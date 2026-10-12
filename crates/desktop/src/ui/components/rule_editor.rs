use copy_for_ai_core::EntityKind;
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::Button,
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
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Custom sensitive terms"),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(div().flex_1().child(Input::new(input)))
                .child(
                    Button::new("kind")
                        .label(kind.tag())
                        .disabled(busy)
                        .on_click(on_cycle),
                )
                .child(
                    Button::new("add-rule")
                        .label("Add")
                        .disabled(busy)
                        .on_click(on_add),
                ),
        )
        .child(
            div()
                .id("rules-list")
                .max_h(px(85.))
                .overflow_y_scroll()
                .children(rows),
        )
}
