use super::{MainTab, ShieldView, settings::SettingsPage};
use crate::application::Appearance;
use gpui_kit::{
    AppContext, Bounds, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Pixels, ScrollDelta,
    ScrollWheelEvent, WindowOptions, point, px, size,
};
use gpui_test::{self as gpui, TestAppContext, VisualTestContext};

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn visible_bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let bounds = cx.debug_bounds(selector).expect(selector);
    let viewport = cx.update(|window, _| window.viewport_size());
    assert!(bounds.size.width > px(0.), "{selector}: {bounds:?}");
    assert!(bounds.size.height > px(0.), "{selector}: {bounds:?}");
    let viewport = Bounds::new(point(px(0.), px(0.)), viewport);
    assert!(
        viewport.contains(&bounds.origin) && viewport.contains(&bounds.bottom_right()),
        "{selector} is outside the window: {bounds:?}, viewport: {viewport:?}"
    );
    bounds
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = visible_bounds(cx, selector);
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn settings_navigation_and_resize_preserve_editors(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new_preview(window, cx))
        })
        .unwrap()
    });
    let cx = &mut VisualTestContext::from_window(handle, cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.show_preview(window, cx);
            this.busy = true;
        });
    });
    let (input, output) = cx.update(|_, cx| {
        let this = view.read(cx);
        (this.input.clone(), this.output.clone())
    });
    for appearance in [Appearance::Light, Appearance::Dark] {
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                let preferences = this.settings.preferences().with_appearance(appearance);
                this.settings.accept(preferences);
                ShieldView::apply_appearance(preferences, window, cx);
            });
        });
        for width in [640., 768., 959., 960., 1024., 1120., 1440.] {
            cx.simulate_resize(size(px(width), px(700.)));
            draw(cx);
            for selector in ["main-0", "main-1", "main-2", "main-3"] {
                visible_bounds(cx, selector);
            }
            click(cx, "main-3");
            for (selector, page) in [
                ("General", SettingsPage::General),
                ("Appearance", SettingsPage::Appearance),
                ("Terms", SettingsPage::Rules),
                ("Sessions and data", SettingsPage::Data),
                ("Applications", SettingsPage::Applications),
                ("Updates", SettingsPage::Updates),
                ("About", SettingsPage::About),
            ] {
                click(cx, selector);
                visible_bounds(cx, "settings-content");
                cx.update(|_, cx| {
                    let this = view.read(cx);
                    assert!(this.settings_page == page);
                    assert!(this.busy);
                    assert_eq!(this.input, input);
                    assert_eq!(this.output, output);
                });
            }
            click(cx, "General");
            let settings = visible_bounds(cx, "settings-content");
            cx.simulate_event(ScrollWheelEvent {
                position: settings.center(),
                delta: ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                ..Default::default()
            });
            draw(cx);
            let shortcuts = visible_bounds(cx, "Global shortcuts");
            assert!(
                settings.contains(&shortcuts.origin)
                    && settings.contains(&shortcuts.bottom_right()),
                "last settings row cannot be scrolled into view at width {width}"
            );
            cx.update(|window, cx| {
                window.blur(cx);
                window.focus_next(cx);
            });
            draw(cx);
            for _ in 0..5 {
                cx.simulate_keystrokes("tab");
                draw(cx);
            }
            let keystroke = Keystroke::parse("enter").unwrap();
            cx.simulate_event(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            cx.simulate_event(KeyUpEvent { keystroke });
            draw(cx);
            cx.update(|_, cx| {
                assert!(view.read(cx).settings_page == SettingsPage::Appearance);
            });
            click(cx, "main-1");
            let table = visible_bounds(cx, "history-table-scroll");
            cx.simulate_event(ScrollWheelEvent {
                position: table.center(),
                delta: ScrollDelta::Pixels(point(px(-1000.), px(0.))),
                ..Default::default()
            });
            draw(cx);
            visible_bounds(cx, "replacement-column");
            let history = cx.debug_bounds("session-history").unwrap();
            assert!(history.size.height >= px(80.));
            click(cx, "main-2");
            visible_bounds(cx, "lifetime-statistics");
            let panel = cx.debug_bounds("outcome-panel").unwrap();
            let legend = cx.debug_bounds("outcome-legend").unwrap();
            let metrics = cx.debug_bounds("operation-metrics").unwrap();
            assert!(metrics.bottom_right().y <= panel.origin.y);
            assert!(panel.size.height >= px(240.), "chart clipped: {panel:?}");
            assert!(
                panel.contains(&legend.origin) && panel.contains(&legend.bottom_right()),
                "legend escapes chart panel at width {width}: {panel:?}, {legend:?}"
            );
            visible_bounds(cx, "status-bar");
            click(cx, "main-0");
            let source = visible_bounds(cx, "source-text");
            let result = visible_bounds(cx, "result-text");
            assert!(source.size.height >= px(80.));
            assert!(result.size.height >= px(80.));
            assert!(
                source.bottom_right().x <= result.origin.x
                    || source.bottom_right().y <= result.origin.y,
                "editors overlap at width {width}"
            );
            for selector in ["cancel", "process-clipboard", "process-input"] {
                visible_bounds(cx, selector);
            }
            cx.update(|_, cx| {
                let this = view.read(cx);
                assert!(this.tab == MainTab::Text);
                assert!(!input.read(cx).value().is_empty());
                assert!(!output.read(cx).value().is_empty());
            });
        }
    }
}
