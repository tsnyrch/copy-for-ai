use super::{ShieldView, processing::Purpose};
use crate::{
    application::{FailureReason, OperationStatus},
    platform::{PastePlatform, Snapshot},
};
use copy_for_ai_core::{Detector, Direction, EntityKind};
use gpui_kit::{AppContext, ClipboardItem, WindowOptions};
use gpui_test::{self as gpui, TestAppContext};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicUsize, Ordering},
};
use zeroize::Zeroizing;

struct TestPlatform {
    trusted: AtomicBool,
    released: AtomicBool,
    target: AtomicI32,
    version: AtomicI64,
    sent: AtomicUsize,
    fail_paste: AtomicBool,
}

impl TestPlatform {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            trusted: AtomicBool::new(true),
            released: AtomicBool::new(true),
            target: AtomicI32::new(42),
            version: AtomicI64::new(1),
            sent: AtomicUsize::new(0),
            fail_paste: AtomicBool::new(false),
        })
    }
}

impl PastePlatform for TestPlatform {
    fn accessibility_trusted(&self) -> bool {
        self.trusted.load(Ordering::SeqCst)
    }
    fn modifiers_released(&self) -> bool {
        self.released.load(Ordering::SeqCst)
    }
    fn target(&self) -> Option<i32> {
        Some(self.target.load(Ordering::SeqCst))
    }
    fn clipboard_version(&self) -> i64 {
        self.version.load(Ordering::SeqCst)
    }
    fn paste(&self, _: Snapshot) -> Result<(), &'static str> {
        if self.fail_paste.load(Ordering::SeqCst) {
            return Err("Synthetic paste failure.");
        }
        self.sent.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[gpui::test]
fn shortcut_uses_current_clipboard_and_restores_known_and_mixed_tokens(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let native = TestPlatform::new();
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    let unknown = "[[PERSON_DEADBEEF_9]]";
    let token = cx
        .update_window(handle, |_, window, cx| {
            view.update(cx, |this, cx| {
                this.native = native.clone();
                this.workflow
                    .add_rule(EntityKind::Person, "Alice Morgan".into())
                    .unwrap();
                let token = this.workflow.process("Alice Morgan", &[]).unwrap().text;
                // A stale anonymized editor must never become the shortcut's source.
                this.output.update(cx, |state, cx| {
                    state.set_value("old anonymized output", window, cx)
                });
                token
            })
        })
        .unwrap();
    for (source, expected, sends, direction) in [
        (
            token.clone(),
            "Alice Morgan".to_owned(),
            1,
            Direction::Restore,
        ),
        (
            format!("{token} {unknown}"),
            format!("Alice Morgan {unknown}"),
            2,
            Direction::Restore,
        ),
        (
            unknown.to_owned(),
            unknown.to_owned(),
            2,
            Direction::Unresolved,
        ),
    ] {
        cx.update_window(handle, |_, window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(source.clone()));
            view.update(cx, |this, cx| this.protected_paste(window, cx));
        })
        .unwrap();
        let mut finished = false;
        for _ in 0..1000 {
            cx.run_until_parked();
            if cx.update(|cx| !view.read(cx).busy) {
                finished = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(finished, "isolated worker did not complete");
        cx.update(|cx| {
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), expected);
            assert_eq!(native.sent.load(Ordering::SeqCst), sends);
            let this = view.read(cx);
            assert!(this.history.entries().back().unwrap().direction == Some(direction));
            assert_eq!(
                this.history.entries().back().unwrap().input_bytes,
                source.len()
            );
            if direction == Direction::Unresolved {
                assert!(this.history.entries().back().unwrap().status == OperationStatus::Failed);
            } else {
                assert!(
                    this.history.entries().back().unwrap().status == OperationStatus::Completed
                );
            }
            assert_eq!(
                this.output.read(cx).value().as_ref(),
                "old anonymized output"
            );
        });
    }
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            this.forget(window, cx);
            this.forget(window, cx);
            assert!(this.history.entries().is_empty());
        })
    })
    .unwrap();
}

#[gpui::test]
fn final_paste_rejects_cancel_target_clipboard_permission_modifiers_and_stale_vault(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            for case in 0..8 {
                let native = TestPlatform::new();
                this.native = native.clone();
                let input = "test@example.test";
                let pending = this
                    .workflow
                    .snapshot()
                    .finish(&Detector::new(), Zeroizing::new(input.into()), &[])
                    .unwrap();
                let expected = pending.output().text.clone();
                let generation = this.generation;
                this.history.begin(Some(Direction::Anonymize), input.len());
                this.busy = true;
                cx.write_to_clipboard(ClipboardItem::new_string(input.into()));
                match case {
                    0 => this.cancel_processing(cx),
                    1 => native.target.store(99, Ordering::SeqCst),
                    2 => native.version.store(2, Ordering::SeqCst),
                    3 => native.trusted.store(false, Ordering::SeqCst),
                    4 => native.released.store(false, Ordering::SeqCst),
                    5 => this.workflow.new_session().unwrap(),
                    6 => native.fail_paste.store(true, Ordering::SeqCst),
                    _ => {}
                }
                let before = this.workflow.entity_count();
                this.deliver_paste(
                    generation,
                    pending,
                    Snapshot {
                        target: 42,
                        clipboard_version: 1,
                    },
                    cx,
                );
                if case == 7 {
                    assert_eq!(native.sent.load(Ordering::SeqCst), 1);
                    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), expected);
                    assert_eq!(this.workflow.entity_count(), before + 1);
                } else {
                    let expected_reason = match case {
                        0 => None,
                        1 => Some(FailureReason::TargetChanged),
                        2 => Some(FailureReason::ClipboardChanged),
                        3 => Some(FailureReason::PermissionMissing),
                        4 => Some(FailureReason::ModifiersPressed),
                        5 => Some(FailureReason::StaleWorkflow),
                        _ => Some(FailureReason::NativeFailure),
                    };
                    assert!(this.history.entries().back().unwrap().failure == expected_reason);
                    if case == 6 {
                        assert!(
                            cx.read_from_clipboard()
                                .and_then(|item| item.text())
                                .unwrap_or_default()
                                .is_empty()
                        );
                    }
                    assert_eq!(native.sent.load(Ordering::SeqCst), 0);
                    assert_eq!(this.workflow.entity_count(), before);
                    if case != 6 {
                        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), input);
                    }
                }
            }
            let failure_native = TestPlatform::new();
            this.native = failure_native.clone();
            let mappings_before = this.workflow.entity_count();
            this.history.begin(Some(Direction::Anonymize), 10);
            this.busy = true;
            this.finish_detection(
                this.generation,
                Err(copy_for_ai_ner::Error::Load),
                Purpose::Protected(Snapshot {
                    target: 42,
                    clipboard_version: 1,
                }),
                window,
                cx,
            );
            assert_eq!(this.workflow.entity_count(), mappings_before);
            assert!(this.error);
            assert!(!this.busy);
            assert_eq!(failure_native.sent.load(Ordering::SeqCst), 0);
            // Plain clipboard processing must never send a paste command.
            let version = this.native.clipboard_version();
            this.complete_processing(
                "test@example.test",
                &[],
                Purpose::Clipboard(version),
                window,
                cx,
            );
        })
    })
    .unwrap();
}

#[gpui::test]
fn clipboard_completion_ignores_target_but_rejects_changed_clipboard_and_revision(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            for case in 0..3 {
                let native = TestPlatform::new();
                this.native = native.clone();
                let input = "test@example.test";
                let pending = this
                    .workflow
                    .snapshot()
                    .finish(&Detector::new(), Zeroizing::new(input.into()), &[])
                    .unwrap();
                let expected = pending.output().text.clone();
                let before = this.workflow.entity_count();
                this.history.begin(Some(Direction::Anonymize), input.len());
                this.busy = true;
                cx.write_to_clipboard(ClipboardItem::new_string(input.into()));
                native.target.store(99, Ordering::SeqCst);
                native.trusted.store(false, Ordering::SeqCst);
                if case == 1 {
                    native.version.store(2, Ordering::SeqCst);
                }
                if case == 2 {
                    this.workflow.new_session().unwrap();
                }
                this.finish_detection(
                    this.generation,
                    Ok(pending),
                    Purpose::Clipboard(1),
                    window,
                    cx,
                );
                assert_eq!(native.sent.load(Ordering::SeqCst), 0);
                if case == 0 {
                    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), expected);
                    assert_eq!(this.workflow.entity_count(), before + 1);
                    assert_eq!(this.workflow.restore(&expected).unwrap().text, input);
                } else {
                    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), input);
                    assert_eq!(this.workflow.entity_count(), before);
                    let reason = if case == 1 {
                        FailureReason::ClipboardChanged
                    } else {
                        FailureReason::StaleWorkflow
                    };
                    assert!(this.history.entries().back().unwrap().failure == Some(reason));
                }
            }
        });
    })
    .unwrap();
}
