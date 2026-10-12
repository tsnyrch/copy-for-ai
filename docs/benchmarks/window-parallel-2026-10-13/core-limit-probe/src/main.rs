use copy_for_ai_core::{Error, Vault};
use std::time::Instant;

fn main() {
    const INPUT_BYTES: usize = 50 * 1024 * 1024;
    let input = "č".repeat(INPUT_BYTES / "č".len());
    if input.len() != INPUT_BYTES {
        std::process::exit(2);
    }
    let vault = Vault::new();
    let started = Instant::now();
    match vault.direction(&input) {
        Err(Error::InputTooLarge) => println!(
            "input_bytes={} error=InputTooLarge elapsed_ns={} ner_invoked=false",
            input.len(),
            started.elapsed().as_nanos()
        ),
        _ => {
            eprintln!("unexpected_preflight_result=true ner_invoked=false");
            std::process::exit(1);
        }
    }
}
