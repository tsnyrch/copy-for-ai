use copy_for_ai_core::{Detector, EntityKind, Vault};
use copy_for_ai_ner::{LocalNer, Metrics, ModelVariant, RuntimeConfig, model_directory};
use std::collections::HashSet;

fn main() {
    let candidate = std::env::args().nth(1).unwrap_or_else(|| "int8".into());
    let variant = match candidate.as_str() {
        "q4" => ModelVariant::Q4,
        "bf16" | "runtime" => ModelVariant::Fp32,
        "int8" => ModelVariant::Int8,
        _ => panic!("unknown evaluation variant"),
    };
    let variants = if candidate == "runtime" {
        vec![ModelVariant::Fp32]
    } else {
        vec![ModelVariant::Fp32, variant]
    };
    let models: Vec<_> = variants
        .into_iter()
        .enumerate()
        .map(|(index, variant)| {
            LocalNer::load_with_backend(
                &model_directory(),
                RuntimeConfig {
                    variant,
                    token_budget: 256,
                    ..Default::default()
                },
                |builder| {
                    if candidate == "bf16" && index == 1 {
                        builder.with_config_entry("mlas.enable_gemm_fastmath_arm64_bfloat16", "1")
                    } else {
                        Ok(builder)
                    }
                },
                || false,
                &mut Metrics::default(),
            )
            .unwrap()
        })
        .collect();
    let samples = [
        ("Petr Dvořák pracuje pro Microsoft v Praze.", "Petr Dvořák"),
        ("Alice Morgan works at Microsoft in London.", "Alice Morgan"),
        (
            "Lena Fischer arbeitet bei Microsoft in Berlin.",
            "Lena Fischer",
        ),
        (
            "Claire Dubois travaille chez Microsoft à Paris.",
            "Claire Dubois",
        ),
        (
            "Анна Петрова работает в Microsoft в Москве.",
            "Анна Петрова",
        ),
        ("田中太郎は東京でMicrosoftに勤務しています。", "田中太郎"),
        ("张伟在北京的Microsoft工作。", "张伟"),
        (
            "Meeting with Alice Morgan, Claire Dubois in Paris.\nowner,company,city\nAlice Morgan,Microsoft,Paris\n",
            "Claire Dubois",
        ),
        (
            "Petr Cup Litomyšl\n* setkání zákazníků společnosti Orion Group\n* Vega Systems staví pobočku - informace Tomáš a řeší Adam Horák\nVega a.s.\nEva Veselá\nLitomyšl",
            "Eva Veselá",
        ),
        (
            r#"{"ItemGUID":"abcd1234-1234-1234-1234-abcdef012345","Title":"Meeting with Alice Morgan and Claire Dubois","Cost":329.0,"Date":"2026-10-12","Owner":"Alice Morgan","Vendor":"Microsoft"}"#,
            "Alice Morgan",
        ),
    ];
    let detector = Detector::new();
    let mut total_missing = 0;
    let mut golden_missing = [0_usize; 2];
    let mut total_added = 0;
    let extra = [
        (
            "Tomáš a Adam Horák jednají s Evou Veselou.",
            "Tomáš",
            EntityKind::Person,
        ),
        ("Li met Bo in London.", "Bo", EntityKind::Person),
        (
            "Orion Group pořádá Petr Cup Litomyšl.",
            "Orion Group",
            EntityKind::Organization,
        ),
        (
            "Orion Group pořádá Petr Cup Litomyšl.",
            "Petr Cup Litomyšl",
            EntityKind::Event,
        ),
        (
            "Alice Morgan works at Microsoft in London.",
            "Microsoft",
            EntityKind::Organization,
        ),
        (
            "Alice Morgan works at Microsoft in London.",
            "London",
            EntityKind::Location,
        ),
        (
            "Send the parcel to 221B Baker Street, London.",
            "221B Baker Street",
            EntityKind::Address,
        ),
        (
            "Petr Dvořák bydlí na adrese Dlouhá 12, Praha.",
            "Dlouhá 12",
            EntityKind::Address,
        ),
        (
            "Анна Петрова живёт по адресу улица Ленина 10, Москва.",
            "улица Ленина 10",
            EntityKind::Address,
        ),
        (
            "田中太郎は東京都渋谷区神南1丁目に住んでいます。",
            "東京都渋谷区神南1丁目",
            EntityKind::Address,
        ),
        (
            "张伟住在北京市朝阳区建国路88号。",
            "北京市朝阳区建国路88号",
            EntityKind::Address,
        ),
    ];
    let samples = samples
        .into_iter()
        .map(|(text, name)| (text, name, EntityKind::Person))
        .chain(extra);
    let mut cases = 0;
    let gate = std::env::args().any(|arg| arg == "gate");
    let prefixes = if gate {
        [0, 27, 31, 35]
    } else {
        [0, 180, 210, 240]
    };
    let mut lost_golden = 0;
    let mut lost_masking = 0;
    let mut masking_missing = [0_usize; 2];
    for (index, (sample, name, kind)) in samples.enumerate() {
        for prefix in prefixes {
            let text = format!(
                "{}\n{sample}\n{sample}",
                "The team reviewed the report. ".repeat(prefix)
            );
            let mut results = Vec::new();
            let mut expected = Vec::new();
            let mut masked = Vec::new();
            for (model_index, model) in models.iter().enumerate() {
                let entities = model.detect(&text, || false).unwrap();
                let start = text.find(name).unwrap();
                let found = entities
                    .iter()
                    .any(|e| e.kind == kind && e.range == (start..start + name.len()));
                let covered = entities
                    .iter()
                    .any(|e| e.range.start <= start && e.range.end >= start + name.len());
                expected.push(found);
                masked.push(covered);
                if !found {
                    golden_missing[model_index] += 1;
                }
                if !covered {
                    masking_missing[model_index] += 1;
                }
                let mut vault = Vault::new();
                let output = vault.process(&detector, &text, &[], &entities).unwrap();
                assert!(
                    vault.restore(&output.text).unwrap().text == text,
                    "round_trip_case={index}"
                );
                results.push(
                    entities
                        .iter()
                        .map(|e| (e.range.start, e.range.end, e.kind))
                        .collect::<HashSet<_>>(),
                );
            }
            if candidate == "runtime" {
                cases += 1;
                println!(
                    "case={index} prefix={prefix} bytes={} entities={} golden={} covered={} round_trip=true",
                    text.len(),
                    results[0].len(),
                    expected[0],
                    masked[0]
                );
                continue;
            }
            let missing = results[0].difference(&results[1]).count();
            total_missing += missing;
            let added = results[1].difference(&results[0]).count();
            total_added += added;
            lost_golden += usize::from(expected[0] && !expected[1]);
            lost_masking += usize::from(masked[0] && !masked[1]);
            cases += 1;
            println!(
                "case={index} prefix={prefix} bytes={} fp32={} candidate={} missing={missing} added={added} fp32_golden={} candidate_golden={} fp32_masked={} candidate_masked={}",
                text.len(),
                results[0].len(),
                results[1].len(),
                expected[0],
                expected[1],
                masked[0],
                masked[1]
            );
        }
    }
    if candidate == "runtime" {
        println!(
            "summary cases={cases} golden_missing={} masking_missing={} round_trip=true",
            golden_missing[0], masking_missing[0]
        );
        return;
    }
    println!(
        "cases={cases} missing={total_missing} added={total_added} fp32_golden_missing={} candidate_golden_missing={} lost_golden={lost_golden} fp32_masking_missing={} candidate_masking_missing={} lost_masking={lost_masking}",
        golden_missing[0], golden_missing[1], masking_missing[0], masking_missing[1]
    );
    if gate {
        let tail = "\nPetr Dvořák pracuje pro Microsoft v Praze.\nAlice Morgan works at Microsoft in London.\nАнна Петрова работает в Microsoft в Москве.\n田中太郎は東京でMicrosoftに勤務しています。\n张伟在北京的Microsoft工作。\nOrion Group pořádá Petr Cup Litomyšl.\nSend the parcel to 221B Baker Street, London.";
        let mut text =
            "The team reviewed report. ".repeat(copy_for_ai_core::MAX_INPUT_BYTES / 26 + 1);
        text.truncate(copy_for_ai_core::MAX_INPUT_BYTES - tail.len());
        text.push_str(tail);
        for (index, model) in models.iter().enumerate() {
            let mut metrics = Metrics::default();
            let entities = model
                .detect_measured(&text, || false, &mut metrics)
                .unwrap();
            let mut found = 0;
            for name in [
                "Petr Dvořák",
                "Alice Morgan",
                "Анна Петрова",
                "田中太郎",
                "张伟",
                "Microsoft",
                "Orion Group",
                "Petr Cup Litomyšl",
                "221B Baker Street",
            ] {
                let start = text.rfind(name).unwrap();
                found += usize::from(
                    entities
                        .iter()
                        .any(|e| e.range.start <= start && e.range.end >= start + name.len()),
                );
            }
            let mut vault = Vault::new();
            let output = vault.process(&detector, &text, &[], &entities).unwrap();
            assert!(
                vault.restore(&output.text).unwrap().text == text,
                "long_round_trip"
            );
            println!(
                "long model={index} bytes={} windows={} inferred={} reused={} tail_expected=9 tail_masked={found} round_trip=true",
                text.len(),
                metrics.windows,
                metrics.inferred_windows,
                metrics.reused_windows
            );
        }
    }
}
