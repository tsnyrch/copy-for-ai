use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run_document(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_copy-for-ai-cli"))
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn full_document_unknown_markers_are_not_published_or_sent_to_a_model() {
    let input = "[[PERSON_12345678_1]]\n田中太郎 / Анна Петрова\n";
    let output = run_document(input.as_bytes());
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("12345678")
    );
}

#[test]
fn oversized_or_invalid_utf8_document_is_rejected_before_model_loading() {
    let output = run_document(&vec![b'a'; copy_for_ai_core::MAX_INPUT_BYTES + 1]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let output = run_document(&[0xff, 0xfe]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
