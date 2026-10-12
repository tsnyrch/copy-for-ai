use copy_for_ai_core::{DetectedEntity, Detector, EntityKind, Session};

#[test]
fn secret_only_masks_complete_enclosures_and_never_a_marker_itself() {
    let text = "x[[PERSON_A_1]][[PERSON_A_2]]y";
    let seam = text.find("[[PERSON_A_2]]").unwrap();
    let cases = [
        (0..text.len(), true),
        (1..text.len() - 1, true),
        (1..seam, false),
        (2..text.len(), false),
        (0..text.len() - 2, false),
        (0..1, true),
        (text.len() - 1..text.len(), true),
    ];
    for (range, masked) in cases {
        let mut session = Session::new();
        let output = Detector::new()
            .anonymize_with_entities(
                text,
                &mut session,
                &[],
                &[DetectedEntity {
                    range,
                    kind: EntityKind::Secret,
                }],
            )
            .unwrap();
        assert_eq!(!output.findings.is_empty(), masked);
        assert_eq!(session.restore(&output.text).unwrap().text, text);
    }
}
