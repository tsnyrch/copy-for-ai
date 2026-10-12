use super::{ShieldView, processing::Purpose};
use copy_for_ai_core::{DetectedEntity, EntityKind};
use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_test::{self as gpui, TestAppContext};
use zeroize::Zeroizing;

#[gpui::test]
fn preview_auto_restore_old_sessions_and_forget(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.), px(820.)),
                cx,
            ))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            this.custom
                .update(cx, |state, cx| state.set_value("Jan Novák", window, cx));
            this.input.update(cx, |state, cx| {
                state.set_value("Jan Novák writes to jan@example.test.", window, cx)
            });
            this.add_rule(window, cx);
            this.complete_processing(
                "Jan Novák writes to jan@example.test.",
                &[],
                Purpose::Preview,
                window,
                cx,
            );
            let output = this.output.read(cx).value().to_string();
            assert!(!output.contains("Jan Novák"));
            assert!(output.contains("[[PERSON_"));
            assert!(output.contains("[[EMAIL_"));
            assert_eq!(this.workflow.entity_count(), 2);
            this.workflow.new_session().unwrap();
            this.input
                .update(cx, |state, cx| state.set_value(output, window, cx));
            let restored = this.input.read(cx).value().to_string();
            this.complete_processing(&restored, &[], Purpose::Preview, window, cx);
            assert!(!this.busy);
            assert_eq!(
                this.output.read(cx).value().as_ref(),
                "Jan Novák writes to jan@example.test."
            );
            this.forget(window, cx);
            assert_eq!(this.workflow.entity_count(), 2);
            this.forget(window, cx);
            assert_eq!(this.workflow.entity_count(), 0);
            assert!(this.workflow.rules().is_empty());
            assert!(this.input.read(cx).value().is_empty());
            assert!(this.output.read(cx).value().is_empty());
        });
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

#[gpui::test]
fn stale_and_changed_input_detection_cannot_create_mapping(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            let input = "Жанна Морозова";
            let entities = || {
                vec![DetectedEntity {
                    range: 0..input.len(),
                    kind: EntityKind::Person,
                }]
            };
            this.input
                .update(cx, |state, cx| state.set_value(input, window, cx));
            this.busy = true;
            let old_generation = this.generation;
            this.cancel_processing(cx);
            this.finish_detection(
                old_generation,
                Ok(this
                    .workflow
                    .snapshot()
                    .finish(
                        &copy_for_ai_core::Detector::new(),
                        Zeroizing::new(input.to_owned()),
                        &entities(),
                    )
                    .unwrap()),
                Purpose::Preview,
                window,
                cx,
            );
            assert_eq!(this.workflow.entity_count(), 0);
            assert!(this.output.read(cx).value().is_empty());
            this.input.update(cx, |state, cx| {
                state.set_value("changed content", window, cx)
            });
            this.finish_detection(
                this.generation,
                Ok(this
                    .workflow
                    .snapshot()
                    .finish(
                        &copy_for_ai_core::Detector::new(),
                        Zeroizing::new(input.to_owned()),
                        &entities(),
                    )
                    .unwrap()),
                Purpose::Preview,
                window,
                cx,
            );
            assert_eq!(this.workflow.entity_count(), 0);
            assert!(this.output.read(cx).value().is_empty());
            assert!(this.error);
        });
    })
    .unwrap();
}

#[gpui::test]
fn settings_action_without_editor_focus_preserves_work_in_progress(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new_preview(window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            this.input.update(cx, |state, cx| {
                state.set_value("Synthetic work in progress", window, cx)
            });
            this.output.update(cx, |state, cx| {
                state.set_value("Synthetic output", window, cx)
            });
            this.busy = true;
        })
    })
    .unwrap();
    cx.update(|cx| cx.dispatch_action(&crate::OpenSettings));
    cx.run_until_parked();
    cx.update(|cx| {
        let this = view.read(cx);
        assert!(this.tab == super::MainTab::Settings);
        assert!(this.busy);
        assert_eq!(
            this.input.read(cx).value().as_ref(),
            "Synthetic work in progress"
        );
        assert_eq!(this.output.read(cx).value().as_ref(), "Synthetic output");
        assert_eq!(this.generation, 0);
    });
}
