mod application;
mod platform;
mod ui;

use gpui_kit::{prelude::*, *};

gpui_kit::actions!(copy_for_ai, [Quit, OpenSettings]);

fn main() {
    let preview_alias = std::env::current_exe()
        .is_ok_and(|path| path.file_name() == Some(std::ffi::OsStr::new("copy-for-ai-ui-preview")));
    let settings_preview = cfg!(debug_assertions)
        && (preview_alias || std::env::args().any(|arg| arg == "--settings-preview"));
    let redesign_preview = cfg!(debug_assertions)
        && std::env::current_exe().is_ok_and(|path| {
            path.file_name() == Some(std::ffi::OsStr::new("copy-for-ai-redesign-preview"))
        });
    let preview = redesign_preview
        || settings_preview
        || (cfg!(debug_assertions) && std::env::args().any(|arg| arg == "--ui-preview"));
    let app = gpui_kit::application().with_assets(ui::assets::Assets);
    app.on_reopen(|cx| {
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, cx| {
                cx.activate(true);
                window.activate_window();
            });
        }
    });
    app.run(move |cx| {
        gpui_kit::init(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.), px(820.)),
                cx,
            ))),
            window_min_size: Some(size(px(900.), px(700.))),
            titlebar: Some(TitlebarOptions {
                title: Some(
                    if preview {
                        "Copy for AI · UI preview"
                    } else {
                        "Copy for AI"
                    }
                    .into(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| {
                let mut view = if preview {
                    ui::ShieldView::new_preview(window, cx)
                } else {
                    ui::ShieldView::new(window, cx)
                };
                if preview {
                    if settings_preview {
                        view.show_settings_preview(cx);
                    } else {
                        view.show_preview(window, cx);
                    }
                } else {
                    view.install_status_bar(window, cx);
                }
                view
            })
        }) {
            Ok(_) => cx.activate(true),
            Err(error) => {
                eprintln!("Could not open window: {error}");
                cx.quit();
            }
        }
    });
}
