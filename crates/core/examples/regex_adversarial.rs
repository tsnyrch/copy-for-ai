use copy_for_ai_core::{Detector, Session};
use std::time::Instant;

fn main() {
    let detector = Detector::new();
    for size in [64 * 1024, 256 * 1024, 1024 * 1024] {
        for (name, unit, tail) in [
            ("email_missing_domain", "a.", "@-"),
            ("pem_missing_end", "-----BEGIN PRIVATE KEY-----\n", ""),
            (
                "credentials_missing_value",
                "api_key               =\"\n",
                "",
            ),
            ("credential_long_value", "a", ""),
        ] {
            let prefix = if name == "credential_long_value" {
                "password="
            } else {
                ""
            };
            let mut text = String::with_capacity(size);
            text.push_str(prefix);
            while text.len() < size - tail.len() {
                text.push_str(unit);
            }
            text.truncate(size - tail.len());
            text.push_str(tail);
            let mut times = Vec::new();
            let mut replacements = 0;
            for _ in 0..3 {
                let mut session = Session::new();
                let started = Instant::now();
                let output = detector.anonymize(&text, &mut session, &[]).unwrap();
                times.push(started.elapsed().as_micros());
                replacements = output.findings.len();
                assert_eq!(session.restore(&output.text).unwrap().text, text);
                assert_eq!(replacements, usize::from(name == "credential_long_value"));
            }
            times.sort_unstable();
            println!(
                "case={name} bytes={size} median_us={} min_us={} max_us={} replacements={replacements} round_trip=true",
                times[1], times[0], times[2]
            );
        }
    }
}
