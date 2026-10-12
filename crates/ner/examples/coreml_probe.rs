#[cfg(all(target_os = "macos", feature = "coreml"))]
#[path = "../../desktop/src/platform/coreml.rs"]
mod coreml;
use copy_for_ai_ner::model_directory;
use ort::execution_providers::{CoreMLExecutionProvider, ExecutionProvider};
use std::time::Instant;

fn main() {
    println!(
        "available={}",
        CoreMLExecutionProvider::default()
            .is_available()
            .unwrap_or(false)
    );
    let start = Instant::now();
    let result = ort::session::Session::builder()
        .and_then(|b| {
            #[cfg(all(target_os = "macos", feature = "coreml"))]
            if std::env::args().any(|s| s == "mlprogram") {
                return coreml::configure(b);
            }
            b.with_execution_providers([CoreMLExecutionProvider::default()
                .build()
                .error_on_failure()])
        })
        .and_then(|b| b.commit_from_file(model_directory().join("model.onnx")));
    match result {
        Ok(model) => {
            println!("loaded=1 load_ms={}", start.elapsed().as_millis());
            if std::env::args().any(|s| s == "shapes") {
                for input in model.inputs {
                    println!("{} {:?}", input.name, input.input_type);
                }
            }
        }
        Err(error) => eprintln!("Model initialization error: {error}"),
    }
}
