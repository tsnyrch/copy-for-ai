use super::*;
use crate::application::Workflow;
use copy_for_ai_core::EntityKind;

#[test]
fn mailbox_discards_cancelled_and_superseded_generations() {
    let service = NerService::new();
    let value = DetectionProgress {
        phase: DetectionPhase::Detecting,
        total: 10,
        completed: 3,
        reused: 1,
        elapsed: Duration::from_secs(1),
    };
    service.publish(0, value);
    service.cancel();
    assert!(service.take_progress().is_none());
    service.publish(0, value);
    assert!(service.take_progress().is_none());
    let task = service.process(
        Zeroizing::new("synthetic".into()),
        Workflow::new().snapshot(),
    );
    service.forget();
    assert!(matches!(smol::block_on(task).result, Err(Error::Cancelled)));
}

#[test]
fn model_free_restoration_invalidates_cache_without_waiting_for_model_lock() {
    let service = NerService::new();
    let mut workflow = Workflow::new();
    let output = workflow.process("test@example.test", &[]).unwrap();
    let held_model = service.state.lock().unwrap();
    let before = service.cache_epoch.load(Ordering::SeqCst);
    let report = smol::block_on(service.process(Zeroizing::new(output.text), workflow.snapshot()));
    assert!(service.cache_epoch.load(Ordering::SeqCst) > before);
    let restored = workflow.commit(report.result.unwrap()).unwrap();
    assert_eq!(restored.text, "test@example.test");
    assert!(!report.metrics.cached_detection);
    drop(held_model);
}

#[test]
#[ignore = "requires the installed, verified FP32 model"]
fn real_cache_reuses_only_success_and_forget_cancels_inflight_inference() {
    let service = NerService::new();
    let mut workflow = Workflow::new();
    let input = "Alice Morgan works at Microsoft in London.";
    let first = smol::block_on(service.process(Zeroizing::new(input.into()), workflow.snapshot()));
    assert!(!first.metrics.cached_detection);
    workflow.commit(first.result.unwrap()).unwrap();
    workflow.new_session().unwrap();
    workflow.add_rule(EntityKind::Term, "works".into()).unwrap();
    let started = Instant::now();
    let second = smol::block_on(service.process(Zeroizing::new(input.into()), workflow.snapshot()));
    println!(
        "cached_operation_us={} inferred={} reused={}",
        started.elapsed().as_micros(),
        second.metrics.ner.inferred_windows,
        second.metrics.ner.reused_windows
    );
    assert!(second.metrics.cached_detection);
    let output = workflow.commit(second.result.unwrap()).unwrap();
    assert_eq!(workflow.restore(&output.text).unwrap().text, input);
    assert!(!output.text.contains("works"));
    let changed =
        smol::block_on(service.process(Zeroizing::new(format!("{input}!")), workflow.snapshot()));
    assert!(!changed.metrics.cached_detection);
    assert!(changed.result.is_ok());
    let varied: String = (0..1000)
        .map(|index| format!("{input} ref{index} "))
        .collect();
    let task = service.process(Zeroizing::new(varied), workflow.snapshot());
    let running = smol::spawn(task);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            Instant::now() < deadline,
            "inference did not publish progress"
        );
        if service
            .take_progress()
            .is_some_and(|p| p.phase == DetectionPhase::Detecting && p.completed > 0)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    service.forget();
    assert!(matches!(
        smol::block_on(running).result,
        Err(Error::Cancelled)
    ));
    let after_forget =
        smol::block_on(service.process(Zeroizing::new(input.into()), workflow.snapshot()));
    assert!(!after_forget.metrics.cached_detection);
    assert!(after_forget.result.is_ok());
}
