use copy_for_ai_core::Session;
use copy_for_ai_core::{
    DetectedEntity, Detector, EntityKind, MAX_CANDIDATES, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, Vault,
};
use copy_for_ai_ner::{LocalNer, Metrics, RuntimeConfig, model_directory};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("validate") {
        let corpus = PathBuf::from(std::env::args().nth(2).ok_or("missing corpus directory")?);
        let ranges = PathBuf::from(std::env::args().nth(3).ok_or("missing range directory")?);
        if std::env::args().nth(4).is_some() {
            return Err("unexpected validation argument".into());
        }
        return validate(&corpus, &ranges);
    }
    let directory = PathBuf::from(std::env::args().nth(1).ok_or("missing corpus directory")?);
    let budget = std::env::args().nth(2).map_or(Ok(256), |s| s.parse())?;
    let mut loading = Metrics::default();
    let model = LocalNer::load_with_backend(
        &model_directory(),
        RuntimeConfig {
            token_budget: budget,
            ..Default::default()
        },
        Ok,
        || false,
        &mut loading,
    )?;
    println!(
        "load verify_ms={} model_ms={}",
        loading.verification.as_millis(),
        loading.loading.as_millis()
    );
    model.detect("The team reviewed the report.", || false)?;
    let mut files = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| path.extension().is_some_and(|value| value == "txt"));
    files.sort();
    let detector = Detector::new();
    for path in files {
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or("invalid case id")?;
        let text = std::fs::read_to_string(&path)?;
        let mut metrics = Metrics::default();
        let start = Instant::now();
        let entities = model.detect_measured(&text, || false, &mut metrics)?;
        let detection = start.elapsed();
        let mut vault = Vault::new();
        let output = vault.process(&detector, &text, &[], &entities)?;
        if vault.restore(&output.text)?.text != text {
            return Err("round trip failed".into());
        }
        println!(
            "case={id} bytes={} detect_us={} windows={} inferred={} reused={} entities={} round_trip=true",
            text.len(),
            detection.as_micros(),
            metrics.windows,
            metrics.inferred_windows,
            metrics.reused_windows,
            entities.len()
        );
        for entity in entities {
            println!(
                "span case={id} start={} end={} kind={:?}",
                entity.range.start, entity.range.end, entity.kind
            );
        }
    }
    Ok(())
}

fn validate(corpus: &Path, range_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = std::fs::read_dir(corpus)
        .map_err(|_| "cannot read corpus directory")?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|_| "cannot read corpus entry")
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| path.extension().is_some_and(|value| value == "txt"));
    files.sort();
    if files.is_empty() {
        return Err("empty validation corpus".into());
    }
    let detector = Detector::new();
    for path in files {
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| {
                !value.is_empty()
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            })
            .ok_or("invalid validation case id")?;
        let text = read_bounded(&path, MAX_INPUT_BYTES)?;
        let ranges = read_bounded(&range_directory.join(format!("{id}.tsv")), MAX_OUTPUT_BYTES)?;
        let mut entities = Vec::new();
        for line in ranges.lines() {
            if entities.len() == MAX_CANDIDATES {
                return Err("too many validation ranges".into());
            }
            let mut fields = line.split('\t');
            let start = fields
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or("invalid validation range")?;
            let end = fields
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or("invalid validation range")?;
            let kind = match fields.next() {
                Some("person") => EntityKind::Person,
                Some("organization") => EntityKind::Organization,
                Some("location") => EntityKind::Location,
                Some("address") => EntityKind::Address,
                Some("event") => EntityKind::Event,
                _ => return Err("invalid validation category".into()),
            };
            if fields.next().is_some() {
                return Err("invalid validation range".into());
            }
            entities.push(DetectedEntity {
                range: start..end,
                kind,
            });
        }
        let mut vault = Vault::new();
        let output = vault
            .process(&detector, &text, &[], &entities)
            .map_err(|_| "validation processing failed")?;
        let restored = vault
            .restore(&output.text)
            .map_err(|_| "validation restore failed")?;
        if restored.text != text {
            return Err("validation round trip failed".into());
        }
        let mut session = Session::new();
        let direct = detector
            .anonymize_with_entities(&text, &mut session, &[], &entities)
            .map_err(|_| "validation detector failed")?;
        if direct.findings.len() != output.replacements
            || session
                .restore(&direct.text)
                .map_err(|_| "validation detector restore failed")?
                .text
                != text
        {
            return Err("validation detector parity failed".into());
        }
        println!("case={id} entities={} round_trip=true", entities.len());
        for finding in direct.findings {
            println!(
                "core_span case={id} start={} end={} kind={:?}",
                finding.range.start, finding.range.end, finding.kind
            );
        }
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: usize) -> Result<String, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path).map_err(|_| "cannot open validation file")?;
    let mut value = String::new();
    file.take(limit as u64 + 1)
        .read_to_string(&mut value)
        .map_err(|_| "cannot read validation file")?;
    if value.len() > limit {
        return Err("validation file exceeds capacity".into());
    }
    Ok(value)
}
