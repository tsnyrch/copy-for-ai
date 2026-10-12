use gpui_kit::{
    component::{ActiveTheme, Theme},
    *,
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Hsla,
    pub surface: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub primary: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
}

impl Palette {
    pub fn current(cx: &App) -> Self {
        let color = |value| rgb(value).into();
        if cx.theme().is_dark() {
            Self {
                canvas: color(0x111C27),
                surface: color(0x192838),
                ink: color(0xEAF1F6),
                muted: color(0x9CAFBD),
                border: color(0x2D4051),
                primary: color(0x23B49A),
                success: color(0x23B49A),
                warning: color(0xE6B252),
                danger: color(0xEC8589),
            }
        } else {
            Self {
                canvas: color(0xF3F6F8),
                surface: color(0xFFFFFF),
                ink: color(0x172A3A),
                muted: color(0x697A89),
                border: color(0xDCE4EB),
                primary: color(0x087F6D),
                success: color(0x078774),
                warning: color(0xA46A12),
                danger: color(0xC54C54),
            }
        }
    }
}

pub fn configure(cx: &mut App) {
    let palette = Palette::current(cx);
    let dark = cx.theme().is_dark();
    Theme::update(cx, |theme| {
        theme.font_family = ".SystemUIFont".into();
        theme.font_size = px(15.);
        theme.colors.background = palette.surface;
        theme.colors.foreground = palette.ink;
        theme.colors.primary = palette.primary;
        theme.colors.primary_foreground = rgb(if dark { 0x061D21 } else { 0xFFFFFF }).into();
        theme.colors.primary_hover = rgb(if dark { 0x32C5AC } else { 0x076B5C }).into();
        theme.colors.button_primary = palette.primary;
        theme.colors.button_primary_foreground = theme.colors.primary_foreground;
        theme.colors.button_primary_hover = theme.colors.primary_hover;
        theme.colors.button_primary_active = palette.primary;
        theme.tokens.button_primary = palette.primary.into();
        theme.tokens.button_primary_hover = theme.colors.primary_hover.into();
        theme.tokens.button_primary_active = palette.primary.into();
        theme.colors.border = palette.border;
        theme.colors.muted_foreground = palette.muted;
        theme.colors.secondary = palette.canvas;
        theme.colors.danger = palette.danger;
    });
}
