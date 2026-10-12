use copy_for_ai_core::{CustomRule, Detector, EntityKind, Error, MAX_ENTITIES, Session};

#[test]
fn vault_limit_rejects_whole_operation_without_partial_mapping() {
    let detector = Detector::new();
    let mut session = Session::new();
    let input = (0..MAX_ENTITIES)
        .map(|i| format!("user{i}@example.test"))
        .collect::<Vec<_>>()
        .join(" ");
    detector.anonymize(&input, &mut session, &[]).unwrap();
    assert_eq!(session.len(), MAX_ENTITIES);
    let rejected = detector.anonymize("new@example.test", &mut session, &[]);
    assert!(matches!(rejected, Err(Error::VaultFull)));
    assert_eq!(session.len(), MAX_ENTITIES);
    assert!(
        detector
            .anonymize("user0@example.test", &mut session, &[])
            .is_ok()
    );
}

#[test]
fn restore_expansion_is_bounded_before_allocating_originals() {
    let detector = Detector::new();
    let mut session = Session::new();
    let input = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
        "A".repeat(200_000)
    );
    let token = detector.anonymize(&input, &mut session, &[]).unwrap().text;
    assert!(matches!(
        session.restore(&token.repeat(200)),
        Err(Error::OutputTooLarge)
    ));
    assert_eq!(session.restore(&token).unwrap().text, input);
}

#[test]
fn oversized_anonymization_does_not_create_partial_mapping() {
    let detector = Detector::new();
    let mut session = Session::new();
    let rules = [CustomRule::new(EntityKind::Organization, "x").unwrap()];
    assert!(matches!(
        detector.anonymize(&"x ".repeat(600_000), &mut session, &rules),
        Err(Error::InputTooLarge)
    ));
    assert!(session.is_empty());
}
