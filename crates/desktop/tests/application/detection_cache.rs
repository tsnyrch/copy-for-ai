use super::*;
use copy_for_ai_core::EntityKind;
use copy_for_ai_ner::RuntimeConfig;

fn identity() -> DetectionIdentity {
    DetectionIdentity {
        config: RuntimeConfig::default(),
        model_digest: "synthetic-model",
        tokenizer_digest: "synthetic-tokenizer",
        threshold: 0.4_f32.to_bits(),
    }
}

#[test]
fn exact_input_configuration_and_forget_epoch_are_required() {
    let identity = identity();
    let cache = DetectionCache::new(
        "Žan",
        identity,
        vec![DetectedEntity {
            range: 0..4,
            kind: EntityKind::Person,
        }],
        Metrics {
            windows: 5,
            ..Default::default()
        },
        3,
    )
    .unwrap();
    assert!(cache.matches("Žan", identity, 3));
    assert!(!cache.matches("Jan", identity, 3));
    assert!(!cache.matches("Žan", identity, 4));
    assert!(cache.invalidated_by(4));
    assert!(!cache.invalidated_by(3));
    assert!(!cache.invalidated_by(2));
    let mut changed = identity;
    changed.config.token_budget += 1;
    assert!(!cache.matches("Žan", changed, 3));
    changed = identity;
    changed.model_digest = "another-model";
    assert!(!cache.matches("Žan", changed, 3));
    assert_eq!(cache.metrics().reused_windows, 5);
    assert_eq!(cache.metrics().inferred_windows, 0);
    assert!(cache.metrics().inference.is_zero());
}

#[test]
fn oversized_capacity_bypasses_cache_without_changing_detection() {
    let entities = Vec::with_capacity(MAX_CACHE_BYTES / std::mem::size_of::<DetectedEntity>() + 1);
    assert!(DetectionCache::new("a", identity(), entities, Metrics::default(), 0).is_none());
}

#[test]
fn cached_model_ranges_are_reapplied_to_current_rules_and_sessions() {
    use crate::application::Workflow;
    use copy_for_ai_core::Detector;
    let cache = DetectionCache::new(
        "Alice at Acme",
        identity(),
        vec![DetectedEntity {
            range: 0..5,
            kind: EntityKind::Person,
        }],
        Metrics::default(),
        0,
    )
    .unwrap();
    let mut workflow = Workflow::new();
    let first = workflow
        .snapshot()
        .finish(
            &Detector::new(),
            Zeroizing::new("Alice at Acme".into()),
            cache.entities(),
        )
        .unwrap();
    let first = workflow.commit(first).unwrap();
    workflow.new_session().unwrap();
    workflow
        .add_rule(EntityKind::Organization, "Acme".into())
        .unwrap();
    let second = workflow
        .snapshot()
        .finish(
            &Detector::new(),
            Zeroizing::new("Alice at Acme".into()),
            cache.entities(),
        )
        .unwrap();
    let second = workflow.commit(second).unwrap();
    assert_ne!(first.text, second.text);
    assert_eq!(second.replacements, 2);
    assert_eq!(
        workflow.restore(&second.text).unwrap().text,
        "Alice at Acme"
    );
}
