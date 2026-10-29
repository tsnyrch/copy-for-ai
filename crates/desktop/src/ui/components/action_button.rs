use gpui_kit::{component::button::Button, prelude::*, *};

pub fn action_button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .h(px(36.))
        .px(px(14.))
        .rounded(px(5.))
        .font_weight(FontWeight::MEDIUM)
}
