use super::{ShieldView, processing::Purpose};
use copy_for_ai_core::{DetectedEntity, Detector, EntityKind, MAX_INPUT_BYTES};
use gpui_kit::{AppContext, WindowOptions};
use gpui_test::{self as gpui, TestAppContext};
use std::time::Instant;
use zeroize::Zeroizing;

#[gpui::test]
fn complete_mebibyte_reaches_gpui_after_worker_acceptance(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ShieldView::new(window, cx))
        })
        .unwrap()
    });
    let tail = "\nAlice Morgan";
    let mut text = "a ".repeat(MAX_INPUT_BYTES / 2);
    text.truncate(MAX_INPUT_BYTES - tail.len());
    text.push_str(tail);
    let entities = [DetectedEntity {
        range: text.len() - "Alice Morgan".len()..text.len(),
        kind: EntityKind::Person,
    }];
    let snapshot = cx
        .update_window(handle, |_, window, cx| {
            view.update(cx, |this, cx| {
                this.input
                    .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
                this.workflow.snapshot()
            })
        })
        .unwrap();
    let pending = smol::block_on(smol::unblock(move || {
        snapshot
            .finish(&Detector::new(), Zeroizing::new(text), &entities)
            .unwrap()
    }));
    let ready = Instant::now();
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| {
            let queue = ready.elapsed();
            let start = Instant::now();
            this.busy = true;
            this.finish_detection(this.generation, Ok(pending), Purpose::Preview, window, cx);
            let publication = start.elapsed();
            assert_eq!(this.workflow.entity_count(), 1);
            assert!(!this.busy);
            assert!(!this.output.read(cx).value().contains("Alice Morgan"));
            assert!(
                this.workflow
                    .restore(this.output.read(cx).value().as_ref())
                    .unwrap()
                    .text
                    .ends_with(tail)
            );
            eprintln!(
                "gpui_bytes={} queue_us={} publish_us={}",
                MAX_INPUT_BYTES,
                queue.as_micros(),
                publication.as_micros()
            );
        });
    })
    .unwrap();
}
