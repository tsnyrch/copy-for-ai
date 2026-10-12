use super::*;
use crate::application::OperationHistory;
use std::time::SystemTime;

#[test]
fn lifetime_survives_history_eviction_restart_and_explicit_reset() {
    let path = std::env::temp_dir()
        .join(format!(
            "copy-for-ai-statistics-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("statistics.conf");
    let (store, loaded) = StatisticsStore::load_path(path.clone()).unwrap();
    let mut history = OperationHistory::with_lifetime(loaded);
    for _ in 0..105 {
        history.begin(Some(Direction::Anonymize), 100);
        history.finish(OperationStatus::Completed, 2);
        history.finish(OperationStatus::Failed, 0);
    }
    history.begin(Some(Direction::Restore), 50);
    history.finish(OperationStatus::Cancelled, 0);
    history.begin(Some(Direction::Unresolved), 30);
    history.finish(OperationStatus::Failed, 0);
    assert_eq!(history.entries().len(), 100);
    let totals = history.lifetime();
    assert_eq!(totals.operations(), 107);
    assert_eq!(totals.completed, 105);
    assert_eq!(totals.cancelled, 1);
    assert_eq!(totals.failed, 1);
    assert_eq!(totals.replacements, 210);
    assert_eq!(totals.input_bytes, 10_580);
    smol::block_on(store.save(totals)).unwrap();
    let (_, reloaded) = StatisticsStore::load_path(path.clone()).unwrap();
    let mut restarted = OperationHistory::with_lifetime(reloaded);
    assert!(restarted.entries().is_empty());
    assert!(restarted.lifetime() == totals);
    let delayed = store.save(restarted.lifetime());
    restarted.clear();
    smol::block_on(store.save(restarted.lifetime())).unwrap();
    smol::block_on(delayed).unwrap();
    assert!(StatisticsStore::load_path(path.clone()).unwrap().1 == LifetimeStatistics::default());
    let mut invalid = totals.encode();
    invalid.push_str("document=unexpected\n");
    fs::write(&path, invalid).unwrap();
    assert!(StatisticsStore::load_path(path.clone()).is_err());
    fs::write(&path, "x".repeat(1_025)).unwrap();
    assert!(StatisticsStore::load_path(path.clone()).is_err());
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
