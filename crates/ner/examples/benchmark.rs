#[cfg(all(target_os = "macos", feature = "coreml"))]
#[path = "../../desktop/src/platform/coreml.rs"]
mod coreml;
use copy_for_ai_core::{Detector, Vault};
use copy_for_ai_ner::{LocalNer, Metrics, ModelVariant, RuntimeConfig, model_directory};
use std::time::{Duration, Instant};

fn synthetic(bytes: usize, varied: bool) -> String {
    let paragraph = "Alice Morgan works at Microsoft in London. The team reviewed the report and prepared the next delivery. ";
    let tail = "\nPetr Dvořák pracuje pro Microsoft v Praze.";
    let mut text = String::new();
    let mut sequence = 0;
    while text.len() < bytes {
        text.push_str(paragraph);
        if varied {
            text.push_str(&format!(" reportref{sequence} "));
        }
        sequence += 1;
    }
    text.truncate(bytes - tail.len());
    text.push_str(tail);
    text
}

fn main() {
    let bytes: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1024".into())
        .parse()
        .unwrap();
    let mut loading = Metrics::default();
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|s| s == "static")
        || (args.iter().any(|s| s == "mlprogram")
            && !cfg!(all(target_os = "macos", feature = "coreml")))
    {
        eprintln!(
            "Unsupported benchmark configuration. CoreML requires the coreml feature; the static experiment is archived."
        );
        std::process::exit(2);
    }
    let config = RuntimeConfig {
        token_budget: args.get(2).map_or(256, |s| s.parse().unwrap()),
        batch_size: args.get(3).map_or(1, |s| s.parse().unwrap()),
        threads: args.get(4).map_or(8, |s| s.parse().unwrap()),
        profile: args.iter().any(|s| s == "profile"),
        coreml: args.iter().any(|s| s == "coreml"),
        variant: match args.get(5).map(String::as_str) {
            Some("int8") => ModelVariant::Int8,
            Some("q4") => ModelVariant::Q4,
            _ => ModelVariant::Fp32,
        },
    };
    let configure = |builder: ort::session::builder::SessionBuilder| {
        if args.iter().any(|s| s == "bf16") {
            return builder.with_config_entry("mlas.enable_gemm_fastmath_arm64_bfloat16", "1");
        }
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        if args.iter().any(|s| s == "mlprogram") {
            return coreml::configure(builder);
        }
        Ok(builder)
    };
    let model = LocalNer::load_with_backend(
        &model_directory(),
        config,
        configure,
        || false,
        &mut loading,
    )
    .unwrap();
    println!(
        "load verify_ms={} model_ms={}",
        loading.verification.as_millis(),
        loading.loading.as_millis()
    );
    let varied = args.iter().any(|s| s == "varied");
    let text = synthetic(bytes, varied);
    let runs = if args.iter().any(|s| s == "cancel-only") {
        0
    } else if varied && bytes >= 1024 * 1024 {
        model
            .detect("The team reviewed the report.", || false)
            .unwrap();
        1
    } else {
        2
    };
    let detector = Detector::new();
    for run in 0..runs {
        let mut metrics = Metrics::default();
        let start = Instant::now();
        let result = model.detect_measured(&text, || false, &mut metrics);
        let detection = start.elapsed();
        let mut core = Duration::ZERO;
        let mut restore = Duration::ZERO;
        let mut count = 0;
        let status = match result {
            Ok(entities) => {
                count = entities.len();
                let mut vault = Vault::new();
                let start = Instant::now();
                let output = vault.process(&detector, &text, &[], &entities);
                core = start.elapsed();
                match output {
                    Ok(output) => {
                        let start = Instant::now();
                        let status = match vault.restore(&output.text) {
                            Ok(restored) => {
                                assert!(restored.text == text, "round_trip_failed");
                                "ok"
                            }
                            Err(copy_for_ai_core::Error::InputTooLarge) => "restore_input_limit",
                            Err(copy_for_ai_core::Error::OutputTooLarge) => "restore_output_limit",
                            Err(_) => "restore_error",
                        };
                        restore = start.elapsed();
                        status
                    }
                    Err(_) => "core_error",
                }
            }
            Err(copy_for_ai_ner::Error::Timeout) => "timeout",
            Err(copy_for_ai_ner::Error::InputTooLarge) => "input_limit",
            Err(_) => "model_error",
        };
        println!(
            "run={run} bytes={bytes} status={status} detect_ms={} windows={} inferred={} reused={} window_ms={} tokenize_ms={} pre_ms={} infer_ms={} decode_ms={} entities={count} core_ms={} restore_ms={} bytes_s={:.0}",
            detection.as_millis(),
            metrics.windows,
            metrics.inferred_windows,
            metrics.reused_windows,
            metrics.windowing.as_millis(),
            metrics.tokenization.as_millis(),
            metrics.preprocessing.as_millis(),
            metrics.inference.as_millis(),
            metrics.decoding.as_millis(),
            core.as_millis(),
            restore.as_millis(),
            bytes as f64 / detection.as_secs_f64()
        );
        if status != "ok" {
            break;
        }
    }
    if config.profile {
        println!("profile_file={}", model.finish_profiling().unwrap());
    }
    let start = Instant::now();
    let result = model.detect(&text, || start.elapsed() >= Duration::from_millis(250));
    println!(
        "cancel_ms={} cancelled={}",
        start.elapsed().as_millis(),
        matches!(result, Err(copy_for_ai_ner::Error::Cancelled))
    );
}
