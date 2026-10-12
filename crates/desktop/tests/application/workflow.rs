use super::*;
use copy_for_ai_core::{DetectedEntity, EntityKind, MAX_INPUT_BYTES};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

#[test]
fn worker_draft_is_discarded_after_rules_or_vault_change() {
    let mut workflow = Workflow::new();
    let text = "Alice Morgan";
    let entities = [DetectedEntity {
        range: 0..text.len(),
        kind: EntityKind::Person,
    }];
    let pending = workflow
        .snapshot()
        .finish(
            &copy_for_ai_core::Detector::new(),
            Zeroizing::new(text.into()),
            &entities,
        )
        .unwrap();
    assert_eq!(workflow.entity_count(), 0);
    workflow
        .add_rule(EntityKind::Organization, "Acme".into())
        .unwrap();
    assert!(workflow.commit(pending).is_err());
    assert_eq!(workflow.entity_count(), 0);
    let pending = workflow
        .snapshot()
        .finish(
            &copy_for_ai_core::Detector::new(),
            Zeroizing::new(text.into()),
            &entities,
        )
        .unwrap();
    workflow.forget();
    assert!(workflow.commit(pending).is_err());
    assert_eq!(workflow.entity_count(), 0);
}

#[test]
fn background_handoff_commits_a_complete_mebibyte_without_copying_the_vault_again() {
    let mut workflow = Workflow::new();
    let tail = "\nAlice Morgan";
    let mut text = "a ".repeat(MAX_INPUT_BYTES / 2);
    text.truncate(MAX_INPUT_BYTES - tail.len());
    text.push_str(tail);
    let entities = [DetectedEntity {
        range: text.len() - "Alice Morgan".len()..text.len(),
        kind: EntityKind::Person,
    }];
    let snapshot = workflow.snapshot();
    let input = Zeroizing::new(text);
    let start = Instant::now();
    let (pending, ready) = smol::block_on(smol::unblock(move || {
        let pending = snapshot
            .finish(&copy_for_ai_core::Detector::new(), input, &entities)
            .unwrap();
        (pending, Instant::now())
    }));
    let core = ready.duration_since(start);
    let queue = ready.elapsed();
    let start = Instant::now();
    let output = workflow.commit(pending).unwrap();
    let commit = start.elapsed();
    assert_eq!(output.replacements, 1);
    assert_eq!(workflow.entity_count(), 1);
    let restored = workflow.restore(&output.text).unwrap();
    assert_eq!(restored.text.len(), MAX_INPUT_BYTES);
    assert!(restored.text.ends_with(tail));
    eprintln!(
        "bytes={} core_us={} handoff_us={} commit_us={}",
        MAX_INPUT_BYTES,
        core.as_micros(),
        queue.as_micros(),
        commit.as_micros()
    );
}

#[test]
fn cancelling_a_queued_worker_does_not_load_or_commit_a_model() {
    let service = NerService::new();
    let workflow = Workflow::new();
    let future = service.process(
        Zeroizing::new("[[PERSON_12345678_1]]".into()),
        workflow.snapshot(),
    );
    service.cancel();
    let start = Instant::now();
    let result = smol::block_on(future);
    assert!(matches!(
        result.result,
        Err(copy_for_ai_ner::Error::Cancelled)
    ));
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(workflow.entity_count(), 0);
}
