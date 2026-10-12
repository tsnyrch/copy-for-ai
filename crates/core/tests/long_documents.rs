use copy_for_ai_core::{
    CustomRule, DetectedEntity, Detector, EntityKind, Error, MAX_ENTITIES, MAX_INPUT_BYTES,
    MAX_VAULT_BYTES, Vault,
};

#[test]
fn one_mebibyte_preserves_utf8_tail_and_repeated_surfaces() {
    let tail = "\nPetr Dvořák / Alice Morgan / Анна Петрова / 田中太郎 / 张伟\n";
    let mut input = "a ".repeat(MAX_INPUT_BYTES / 2);
    input.truncate(MAX_INPUT_BYTES - tail.len());
    input.push_str(tail);
    let entities: Vec<_> = [
        "Petr Dvořák",
        "Alice Morgan",
        "Анна Петрова",
        "田中太郎",
        "张伟",
    ]
    .into_iter()
    .map(|name| {
        let start = input.find(name).unwrap();
        DetectedEntity {
            range: start..start + name.len(),
            kind: EntityKind::Person,
        }
    })
    .collect();
    let mut vault = Vault::new();
    let output = vault
        .process(&Detector::new(), &input, &[], &entities)
        .unwrap();
    assert_eq!(output.replacements, 5);
    assert!(vault.restore(&output.text).unwrap().text == input);
    assert!(matches!(
        vault.direction(&(input + "a")),
        Err(Error::InputTooLarge)
    ));
}

#[test]
fn expanded_anonymized_document_can_be_restored_through_auto_direction() {
    let input = "x ".repeat(MAX_INPUT_BYTES / 2);
    let rules = [CustomRule::new(EntityKind::Person, "x").unwrap()];
    let mut vault = Vault::new();
    let detector = Detector::new();
    let output = vault.process(&detector, &input, &rules, &[]).unwrap();
    assert!(output.text.len() > MAX_INPUT_BYTES);
    assert_eq!(output.replacements, MAX_INPUT_BYTES / 2);
    assert_eq!(vault.entity_count(), 1);
    let restored = vault.process(&detector, &output.text, &[], &[]).unwrap();
    assert!(restored.text == input);
}

#[test]
fn vault_bytes_and_cross_session_capacity_fail_atomically() {
    let detector = Detector::new();
    let mut vault = Vault::new();
    let body = 900_000;
    for index in 0..MAX_VAULT_BYTES / body {
        let input = format!(
            "-----BEGIN PRIVATE KEY-----\n{index}{}\n-----END PRIVATE KEY-----",
            "A".repeat(body)
        );
        vault.process(&detector, &input, &[], &[]).unwrap();
    }
    let count = vault.entity_count();
    let input = format!(
        "new@example.test -----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
        "B".repeat(body)
    );
    assert!(matches!(
        vault.process(&detector, &input, &[], &[]),
        Err(Error::VaultBytesFull)
    ));
    assert_eq!(vault.entity_count(), count);

    let mut vault = Vault::new();
    let input = (0..MAX_ENTITIES)
        .map(|i| format!("user{i}@example.test"))
        .collect::<Vec<_>>()
        .join(" ");
    vault.process(&detector, &input, &[], &[]).unwrap();
    vault.new_session().unwrap();
    assert!(matches!(
        vault.process(&detector, "other@example.test", &[], &[]),
        Err(Error::VaultFull)
    ));
    assert_eq!(vault.entity_count(), MAX_ENTITIES);
}

#[test]
fn excessive_unknown_markers_are_bounded_without_mutation() {
    let input = (0..=MAX_ENTITIES)
        .map(|i| format!("[[PERSON_12345678_{i}]]"))
        .collect::<String>();
    let mut vault = Vault::new();
    assert!(matches!(
        vault.process(&Detector::new(), &input, &[], &[]),
        Err(Error::TooManyUnknownTokens)
    ));
    assert_eq!(vault.entity_count(), 0);
}

#[test]
fn cancellation_during_core_search_cannot_create_mapping() {
    let rules = [CustomRule::new(EntityKind::Person, "x").unwrap()];
    let input = "x ".repeat(100_000);
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 128
    };
    let mut vault = Vault::new();
    assert!(matches!(
        vault.process_cancellable(&Detector::new(), &input, &rules, &[], &cancelled),
        Err(Error::Cancelled)
    ));
    assert_eq!(vault.entity_count(), 0);
}
