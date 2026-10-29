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
    pub control_border: Hsla,
    pub primary: Hsla,
    pub primary_ink: Hsla,
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub accent_ink: Hsla,
    pub accent_wash: Hsla,
    pub hover: Hsla,
    pub focus: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
}

impl Palette {
    pub fn current(cx: &App) -> Self {
        let color = |value| rgb(value).into();
        // Signal Orange's four source colors are unchanged. Other roles are
        // application-derived, including the dark companion to Warm Paper.
        let mut palette = Self {
            canvas: color(0xF5F4F0),
            surface: color(0xFFFFFF),
            ink: color(0x0B0C0E),
            muted: color(0x65625B),
            border: color(0xDEDCD5),
            control_border: color(0x85827A),
            primary: color(0xFF4D00),
            primary_ink: color(0x0B0C0E),
            primary_hover: color(0xF04A00),
            primary_active: color(0xE54500),
            accent_ink: color(0xC83700),
            accent_wash: color(0xFFF0E8),
            hover: color(0xECEAE4),
            focus: color(0xC83700),
            success: color(0x216343),
            warning: color(0x805300),
            danger: color(0xB42332),
        };
        if cx.theme().is_dark() {
            palette.canvas = color(0x0B0C0E);
            palette.surface = color(0x191A1D);
            palette.ink = color(0xF5F4F0);
            palette.muted = color(0xB4B1A9);
            palette.border = color(0x3E3E40);
            palette.control_border = color(0x85827A);
            palette.accent_ink = color(0xFFAD87);
            palette.accent_wash = color(0x362017);
            palette.hover = color(0x2B2B2E);
            palette.focus = color(0xFFAD87);
            palette.success = color(0x81D5A8);
            palette.warning = color(0xF2C66D);
            palette.danger = color(0xFF9AA4);
        }
        palette
    }
}

pub fn configure(cx: &mut App) {
    let palette = Palette::current(cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Roboto".into();
        theme.font_size = px(14.);
        theme.radius = px(5.);
        theme.radius_lg = px(8.);
        theme.shadow = false;
        theme.focus_ring = true;
        let colors = &mut theme.colors;
        colors.background = palette.surface;
        colors.foreground = palette.ink;
        colors.primary = palette.primary;
        colors.primary_foreground = palette.primary_ink;
        colors.primary_hover = palette.primary_hover;
        colors.primary_active = palette.primary_active;
        colors.button_primary = palette.primary;
        colors.button_primary_foreground = palette.primary_ink;
        colors.button_primary_hover = palette.primary_hover;
        colors.button_primary_active = palette.primary_active;
        colors.button = palette.surface;
        colors.button_foreground = palette.ink;
        colors.button_hover = palette.hover;
        colors.button_active = palette.hover;
        colors.border = palette.border;
        colors.input = palette.control_border;
        colors.muted = palette.canvas;
        colors.muted_foreground = palette.muted;
        colors.secondary = palette.canvas;
        colors.secondary_foreground = palette.ink;
        colors.secondary_hover = palette.hover;
        colors.secondary_active = palette.hover;
        colors.accent = palette.accent_wash;
        colors.accent_foreground = palette.accent_ink;
        colors.ring = palette.focus;
        colors.caret = palette.ink;
        colors.selection = palette.primary.opacity(0.22);
        colors.link = palette.accent_ink;
        colors.link_hover = palette.accent_ink;
        colors.link_active = palette.accent_ink;
        colors.danger = palette.danger;
        colors.danger_hover = palette.danger;
        colors.danger_active = palette.danger;
        colors.success = palette.success;
        colors.warning = palette.warning;
        colors.switch = palette.control_border;
        colors.switch_thumb = palette.surface;
        colors.progress_bar = palette.primary;
        colors.scrollbar = palette.canvas;
        colors.scrollbar_thumb = palette.control_border;
        colors.scrollbar_thumb_hover = palette.muted;
        colors.popover = palette.surface;
        colors.popover_foreground = palette.ink;
        colors.title_bar = palette.surface;
        colors.title_bar_border = palette.border;
        colors.list = palette.surface;
        colors.list_hover = palette.hover;
        colors.list_active = palette.accent_wash;
        colors.list_active_border = palette.focus;
        theme.tokens = theme.colors.into();
    });
}
