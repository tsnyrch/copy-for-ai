use copy_for_ai_core::{CustomRule, Detector, EntityKind, Error, MAX_INPUT_BYTES, Session};

#[test]
fn czech_utf8_round_trip_preserves_text_and_identity() {
    let detector = Detector::new();
    let mut session = Session::new();
    let rules = [
        CustomRule::new(EntityKind::Person, "Jan Novák").unwrap(),
        CustomRule::new(EntityKind::Organization, "Žluťoučká firma").unwrap(),
    ];
    let input =
        "Jan Novák z Žluťoučká firma píše na jan@example.cz. Jan Novák volá +420 777 123 456.";
    let output = detector.anonymize(input, &mut session, &rules).unwrap();
    for original in [
        "Jan Novák",
        "Žluťoučká firma",
        "jan@example.cz",
        "+420 777 123 456",
    ] {
        assert!(!output.text.contains(original));
    }
    assert_eq!(output.findings[0].token, output.findings[3].token);
    assert_eq!(session.len(), 4);
    assert_eq!(session.restore(&output.text).unwrap().text, input);
    let next = detector
        .anonymize("Jan Novák", &mut session, &rules)
        .unwrap();
    assert_eq!(next.text, output.findings[0].token);
}

#[test]
fn existing_tokens_are_protected_and_restore_never_cascades() {
    let detector = Detector::new();
    let mut session = Session::new();
    let output = detector
        .anonymize("one@example.cz", &mut session, &[])
        .unwrap();
    let rules = [CustomRule::new(EntityKind::Term, "EMAIL").unwrap()];
    assert_eq!(
        detector
            .anonymize(&output.text, &mut session, &rules)
            .unwrap()
            .text,
        output.text
    );
    let input = format!("password={}", output.text);
    let second = detector.anonymize(&input, &mut session, &[]).unwrap();
    assert_eq!(second.text, input);
    // Restore a value that itself contains a token, without a second replacement pass.
    let original = format!("secret-before{}", output.text);
    let secret = format!("api_key={original}");
    let masked = detector.anonymize(&secret, &mut session, &[]).unwrap();
    assert!(!masked.text.contains("secret-before"));
    assert_eq!(session.restore(&masked.text).unwrap().text, secret);
}

#[test]
fn foreign_and_malformed_markers_are_not_guessed() {
    let detector = Detector::new();
    let mut first = Session::new();
    let token = detector
        .anonymize("one@example.cz", &mut first, &[])
        .unwrap()
        .text;
    let second = Session::new();
    let restored = second
        .restore(&format!("{token} {token} [[PERSON_BROKEN]]"))
        .unwrap();
    assert_eq!(restored.replacements, 0);
    assert_eq!(restored.unknown_tokens.len(), 2);
    assert!(restored.text.contains(&token));
    assert_ne!(first.scope(), second.scope());
}

#[test]
fn secrets_win_over_nested_email_and_phone() {
    let detector = Detector::new();
    let mut session = Session::new();
    let input = "password=john@example.com api_key=sk-proj-abcdefghijklmnopqrstuvwxyz123456";
    let output = detector.anonymize(input, &mut session, &[]).unwrap();
    assert_eq!(output.findings.len(), 2);
    assert!(output.findings.iter().all(|f| f.kind == EntityKind::Secret));
    assert!(output.text.starts_with("password=[[SECRET_"));
    assert_eq!(session.restore(&output.text).unwrap().text, input);
}

#[test]
fn iban_checksum_and_birth_date_are_validated() {
    let detector = Detector::new();
    let mut session = Session::new();
    let input = "Účet CZ65 0800 0000 1920 0014 5399. RČ 850101/0001. Neplatné: CZ66 0800 0000 1920 0014 5399, 851331/0000.";
    let output = detector.anonymize(input, &mut session, &[]).unwrap();
    assert!(output.findings.iter().any(|f| f.kind == EntityKind::Iban));
    assert!(
        output
            .findings
            .iter()
            .any(|f| f.kind == EntityKind::PersonalId)
    );
    assert!(output.text.contains("CZ66 0800 0000 1920 0014 5399"));
    assert!(output.text.contains("851331/0000"));
    assert_eq!(session.restore(&output.text).unwrap().text, input);
}

#[test]
fn custom_rules_match_whole_unicode_words_and_longest_phrase() {
    let detector = Detector::new();
    let mut session = Session::new();
    let rules = [
        CustomRule::new(EntityKind::Person, "Jan").unwrap(),
        CustomRule::new(EntityKind::Person, "Jan Novák").unwrap(),
    ];
    let output = detector
        .anonymize("Jan Novák, Jan a Jana; Janův.", &mut session, &rules)
        .unwrap();
    assert_eq!(output.findings.len(), 2);
    assert_ne!(output.findings[0].token, output.findings[1].token);
    assert!(output.text.contains("Jana; Janův"));
}

#[test]
fn oversized_input_cannot_mutate_the_vault() {
    let detector = Detector::new();
    let mut session = Session::new();
    let text = "a".repeat(MAX_INPUT_BYTES + 1);
    assert!(matches!(
        detector.anonymize(&text, &mut session, &[]),
        Err(Error::InputTooLarge)
    ));
    assert!(session.restore(&text).is_ok());
    assert!(matches!(
        session.restore(&"a".repeat(copy_for_ai_core::MAX_OUTPUT_BYTES + 1)),
        Err(Error::InputTooLarge)
    ));
    assert!(session.is_empty());
    assert!(CustomRule::new(EntityKind::Term, " ").is_err());
    assert!(CustomRule::new(EntityKind::Term, "[[PERSON_X_1]]").is_err());
}
