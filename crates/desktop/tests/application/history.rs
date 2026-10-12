use super::*;

#[test]
fn bounded_metadata_records_each_terminal_state_once_and_forgets_active_work() {
    let mut history = OperationHistory::new();
    for index in 0..103 {
        history.begin(Some(Direction::Anonymize), index);
        history.finish(OperationStatus::Completed, 2);
        history.finish(OperationStatus::Failed, 0);
    }
    assert_eq!(history.entries().len(), 100);
    assert_eq!(history.entries().front().unwrap().input_bytes, 3);
    history.begin(Some(Direction::Restore), 50);
    history.finish(OperationStatus::Cancelled, 0);
    history.begin(Some(Direction::Unresolved), 40);
    history.finish(OperationStatus::Failed, 0);
    let entries = history.entries();
    assert!(entries[98].status == OperationStatus::Cancelled);
    assert!(entries[99].status == OperationStatus::Failed);
    history.begin(None, 0);
    history.clear();
    history.finish(OperationStatus::Completed, 1);
    assert!(history.entries().is_empty());
}

#[test]
fn scalar_diagnostics_and_failure_survive_only_in_memory_history() {
    let mut history = OperationHistory::new();
    history.begin(Some(Direction::Anonymize), 100);
    history.set_metrics(OperationMetrics {
        ner: copy_for_ai_ner::Metrics {
            windows: 5,
            reused_windows: 3,
            ..Default::default()
        },
        cached_detection: true,
        ..Default::default()
    });
    history.set_paste_wait(Duration::from_millis(17));
    history.set_failure(FailureReason::ClipboardChanged);
    history.finish(OperationStatus::Failed, 0);
    let entry = history.entries().back().unwrap();
    assert_eq!(entry.metrics.as_ref().unwrap().ner.windows, 5);
    assert!(entry.metrics.as_ref().unwrap().cached_detection);
    assert_eq!(
        entry.metrics.as_ref().unwrap().paste_wait,
        Duration::from_millis(17)
    );
    assert!(entry.failure == Some(FailureReason::ClipboardChanged));
    history.clear();
    assert!(history.entries().is_empty());
}
