use copy_for_ai_core::{DetectedEntity, Detector, Direction, EntityKind, Error, Vault};

fn person(text: &str, name: &str) -> DetectedEntity {
    let start = text.find(name).unwrap();
    DetectedEntity {
        range: start..start + name.len(),
        kind: EntityKind::Person,
    }
}

#[test]
fn same_pipeline_preserves_formats_languages_and_utf8() {
    let inputs = [
        (
            "Žaneta Králová píše na info@example.test.\r\n",
            "Žaneta Králová",
        ),
        (
            "{\"unrelated\":\"Анна Петрова\",\"value\":42}",
            "Анна Петрова",
        ),
        ("担当,田中太郎,info@example.test\n", "田中太郎"),
        (
            "log: user=张伟; uuid=abcd1234-1234-1234-1234-abcdef012345",
            "张伟",
        ),
        ("<b>Claire Dubois</b> joined today.", "Claire Dubois"),
    ];
    for (input, name) in inputs {
        let mut vault = Vault::new();
        let output = vault
            .process(&Detector::new(), input, &[], &[person(input, name)])
            .unwrap();
        assert!(!output.text.contains(name));
        assert!(output.text.contains("[[PERSON_"));
        let restored = vault
            .process(&Detector::new(), &output.text, &[], &[])
            .unwrap();
        assert_eq!(restored.direction, Direction::Restore);
        assert_eq!(restored.text, input);
    }
}

#[test]
fn old_sessions_restore_together_without_model_or_current_scope() {
    let mut vault = Vault::new();
    let detector = Detector::new();
    let first = vault
        .process(&detector, "one@example.test", &[], &[])
        .unwrap()
        .text;
    vault.new_session().unwrap();
    let second = vault
        .process(&detector, "two@example.test", &[], &[])
        .unwrap()
        .text;
    assert_ne!(first, second);
    let input = format!("{second} + {first} + [[PERSON_FOREIGN_1]] + raw@example.test");
    let restored = vault.process(&detector, &input, &[], &[]).unwrap();
    assert_eq!(restored.direction, Direction::Restore);
    assert_eq!(restored.replacements, 2);
    assert_eq!(restored.unknown_tokens.len(), 1);
    assert_eq!(
        restored.text,
        "two@example.test + one@example.test + [[PERSON_FOREIGN_1]] + raw@example.test"
    );
    assert_eq!(vault.entity_count(), 2);
    vault.forget();
    assert_eq!(vault.direction(&first).unwrap(), Direction::Unresolved);
}

#[test]
fn unknown_markers_never_create_mappings_or_trigger_anonymization() {
    let mut vault = Vault::new();
    for input in [
        "[[PERSON_FOREIGN_1]] raw@example.test",
        "[[person_foreign_1]] raw@example.test",
    ] {
        let result = vault.process(&Detector::new(), input, &[], &[]).unwrap();
        assert_eq!(result.direction, Direction::Unresolved);
        assert_eq!(result.text, input);
        assert_eq!(vault.entity_count(), 0);
    }
}

#[test]
fn invalid_spans_reject_whole_operation_without_mapping() {
    let mut vault = Vault::new();
    for range in [0..1, std::ops::Range { start: 3, end: 2 }, 0..50, 2..2] {
        let entity = DetectedEntity {
            range,
            kind: EntityKind::Person,
        };
        assert!(matches!(
            vault.process(&Detector::new(), "Žaneta info@example.test", &[], &[entity]),
            Err(Error::InvalidSpan)
        ));
        assert_eq!(vault.entity_count(), 0);
    }
}

#[test]
fn restoration_is_one_pass_even_when_original_contains_marker() {
    let mut vault = Vault::new();
    let detector = Detector::new();
    let input = "password=secret[[PERSON_FOREIGN_1]]";
    // Explicit lower-level API can encode a credential containing a marker.
    let mut session = copy_for_ai_core::Session::new();
    let output = detector.anonymize(input, &mut session, &[]).unwrap();
    assert_eq!(session.restore(&output.text).unwrap().text, input);
    for _ in 1..16 {
        vault.new_session().unwrap();
    }
    assert_eq!(vault.new_session(), Err(Error::TooManySessions));
}

#[test]
fn detected_surface_masks_repeated_mentions_without_format_assumptions() {
    let input = "Alice Morgan meets clients.\nowner,Alice Morgan\n{\"contact\":\"Alice Morgan\"}";
    let mut vault = Vault::new();
    let output = vault
        .process(
            &Detector::new(),
            input,
            &[],
            &[person(input, "Alice Morgan")],
        )
        .unwrap();
    assert_eq!(output.replacements, 3);
    assert_eq!(vault.entity_count(), 1);
    assert!(!output.text.contains("Alice Morgan"));
    assert_eq!(vault.restore(&output.text).unwrap().text, input);
}
