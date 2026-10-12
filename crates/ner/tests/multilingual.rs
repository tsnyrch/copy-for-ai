use copy_for_ai_core::{Detector, Direction, EntityKind, Vault};
use copy_for_ai_ner::{LocalNer, model_directory};

#[test]
#[ignore = "requires installed 1.16 GB pinned model and ONNX runtime"]
fn real_model_multilingual_round_trip() {
    let model = LocalNer::load(&model_directory()).expect("load verified local model");
    let samples = [
        (
            "cs",
            "Petr Dvořák pracuje pro Microsoft v Praze.",
            "Petr Dvořák",
        ),
        (
            "en",
            "Alice Morgan works at Microsoft in London.",
            "Alice Morgan",
        ),
        (
            "de",
            "Lena Fischer arbeitet bei Microsoft in Berlin.",
            "Lena Fischer",
        ),
        (
            "fr",
            "Claire Dubois travaille chez Microsoft à Paris.",
            "Claire Dubois",
        ),
        (
            "ru",
            "Анна Петрова работает в Microsoft в Москве.",
            "Анна Петрова",
        ),
        (
            "ja",
            "田中太郎は東京でMicrosoftに勤務しています。",
            "田中太郎",
        ),
        ("zh", "张伟在北京的Microsoft工作。", "张伟"),
    ];
    let detector = Detector::new();
    let mut successes = 0;
    for (language, text, person) in samples {
        let entities = model.detect(text, || false).expect("local inference");
        let person_found = entities
            .iter()
            .any(|e| e.kind == EntityKind::Person && &text[e.range.clone()] == person);
        let mut vault = Vault::new();
        let output = vault.process(&detector, text, &[], &entities).unwrap();
        assert_eq!(output.direction, Direction::Anonymize);
        assert_eq!(vault.restore(&output.text).unwrap().text, text);
        eprintln!(
            "language={language}; entities={}; expected_person={person_found}",
            entities.len()
        );
        successes += usize::from(person_found);
    }
    assert!(
        successes == 7,
        "model smoke coverage below required 7/7 synthetic cases"
    );
}

#[test]
#[ignore = "requires installed model"]
fn real_model_detects_after_first_window_and_cancelled_input() {
    let model = LocalNer::load(&model_directory()).unwrap();
    let text = format!(
        "{} Alice Morgan works in London.",
        "Ordinary sentence without personal information. ".repeat(30)
    );
    let entities = model.detect(&text, || false).unwrap();
    let start = text.find("Alice Morgan").unwrap();
    assert!(
        entities
            .iter()
            .any(|e| e.kind == EntityKind::Person && e.range.start == start)
    );
    assert!(matches!(
        model.detect(&text, || true),
        Err(copy_for_ai_ner::Error::Cancelled)
    ));
}

#[test]
#[ignore = "requires installed model"]
fn real_model_processes_generic_business_formats() {
    let model = LocalNer::load(&model_directory()).unwrap();
    let detector = Detector::new();
    let examples = [
        (
            r#"{"ItemGUID":"abcd1234-1234-1234-1234-abcdef012345","Title":"Meeting in Prague","Note":"Eva Veselá meets Petr Dvořák from Microsoft.","Contact":"Eva Veselá","City":"Praha"}"#,
            &["Eva Veselá", "Petr Dvořák"][..],
        ),
        (
            "# Meeting notes\nAlice Morgan from Microsoft meets Claire Dubois in Paris.\nowner,company,city\nAlice Morgan,Microsoft,Paris\n",
            &["Alice Morgan", "Claire Dubois"][..],
        ),
    ];
    for (index, (text, names)) in examples.into_iter().enumerate() {
        let entities = model.detect(text, || false).unwrap();
        let mut vault = Vault::new();
        let output = vault.process(&detector, text, &[], &entities).unwrap();
        for name in names {
            assert!(
                !output.text.contains(name),
                "unmasked person in generic format {index}"
            );
        }
        assert_eq!(vault.restore(&output.text).unwrap().text, text);
        eprintln!("format={index}; replacements={}", output.replacements);
    }
}

#[test]
#[ignore = "requires installed model"]
fn real_model_detects_names_events_and_places_in_short_business_notes() {
    let model = LocalNer::load(&model_directory()).unwrap();
    let text = "Petr Cup Litomyšl\n* setkání zákazníků společnosti Orion Group\n* Vega Systems staví pobočku - informace Tomáš a řeší Adam Horák\nVega a.s.\nEva Veselá\nLitomyšl";
    let entities = model.detect(text, || false).unwrap();
    let output = Vault::new()
        .process(&Detector::new(), text, &[], &entities)
        .unwrap();
    for (index, value) in [
        "Petr Cup Litomyšl",
        "Orion Group",
        "Tomáš",
        "Adam Horák",
        "Eva Veselá",
        "Litomyšl",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            !output.text.contains(value),
            "unmasked business entity {index}"
        );
    }
    eprintln!("business-notes; replacements={}", output.replacements);
}

#[test]
#[ignore = "requires installed model"]
fn real_model_covers_entire_mebibyte_including_multilingual_tail() {
    let model = LocalNer::load(&model_directory()).unwrap();
    let tail = "\nAlice Morgan works at Microsoft in London.\nPetr Dvořák pracuje pro Microsoft v Praze.\nАнна Петрова работает в Microsoft в Москве.\n田中太郎は東京でMicrosoftに勤務しています。\n张伟在北京的Microsoft工作。";
    let mut text =
        "The team reviewed the report. ".repeat(copy_for_ai_core::MAX_INPUT_BYTES / 29 + 1);
    text.truncate(copy_for_ai_core::MAX_INPUT_BYTES - tail.len());
    text.push_str(tail);
    let entities = model.detect(&text, || false).unwrap();
    for (index, name) in [
        "Alice Morgan",
        "Petr Dvořák",
        "Анна Петрова",
        "田中太郎",
        "张伟",
    ]
    .into_iter()
    .enumerate()
    {
        let start = text.find(name).unwrap();
        assert!(
            entities
                .iter()
                .any(|e| e.kind == EntityKind::Person && e.range == (start..start + name.len())),
            "tail_case={index}"
        );
    }
    let mut vault = Vault::new();
    let output = vault
        .process(&Detector::new(), &text, &[], &entities)
        .unwrap();
    assert!(vault.restore(&output.text).unwrap().text == text);
    assert!(matches!(
        model.detect(&text, || true),
        Err(copy_for_ai_ner::Error::Cancelled)
    ));
    eprintln!("long_bytes={} entities={}", text.len(), entities.len());
}

#[test]
#[ignore = "requires installed model"]
fn real_model_keeps_names_across_token_window_boundaries_and_batches() {
    use copy_for_ai_ner::{Metrics, RuntimeConfig};
    let model = LocalNer::load_configured(
        &model_directory(),
        RuntimeConfig {
            batch_size: 2,
            ..Default::default()
        },
        || false,
        &mut Metrics::default(),
    )
    .unwrap();
    for (index, prefix) in [27, 29, 31, 33, 35, 37].into_iter().enumerate() {
        let text = format!(
            "{}Alice Morgan works at Microsoft in London.\nPetr Dvořák pracuje pro Microsoft v Praze.",
            "The team reviewed the report. ".repeat(prefix)
        );
        let mut metrics = Metrics::default();
        let entities = model
            .detect_measured(&text, || false, &mut metrics)
            .unwrap();
        assert!(metrics.windows > 1);
        for name in ["Alice Morgan", "Petr Dvořák"] {
            let start = text.find(name).unwrap();
            assert!(entities.iter().any(|e| e.kind == EntityKind::Person && e.range == (start..start + name.len())), "seam_case={index}");
        }
    }
}
