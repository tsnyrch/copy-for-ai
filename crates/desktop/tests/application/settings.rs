use super::*;

#[test]
fn persists_scalar_preferences_and_rejects_unexpected_content() {
    let path = std::env::temp_dir()
        .join(format!(
            "copy-for-ai-preferences-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("preferences.conf");
    let service = SettingsService {
        preferences: Preferences::default(),
        store: Some(path.clone()),
    };
    let preferences = Preferences::default()
        .with_appearance(Appearance::Dark)
        .with_history(true);
    let accepted = smol::block_on(service.save(preferences)).unwrap();
    assert!(accepted == preferences);
    let stored = fs::read_to_string(&path).unwrap();
    assert!(Preferences::decode(&stored).unwrap() == preferences);
    assert!(Preferences::decode(&format!("{stored}document=unexpected\n")).is_err());
    assert!(Preferences::decode("version=1\nappearance=other\nshow_history=false\n").is_err());
    let parent = path.parent().unwrap();
    fs::remove_file(&path).unwrap();
    fs::remove_dir(parent).unwrap();
}
