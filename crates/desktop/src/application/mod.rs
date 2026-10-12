mod shortcuts;
mod workflow;

pub use shortcuts::Shortcuts;
pub use workflow::{PendingProcessing, Workflow};

mod ner;
pub use ner::NerService;

#[cfg(test)]
#[path = "../../tests/application/workflow.rs"]
mod tests;

mod history;
pub use history::{OperationHistory, OperationStatus};

mod settings;
pub use settings::{Appearance, LICENSE_TEXT, Preferences, SettingsService};

mod statistics;
pub use statistics::{LifetimeStatistics, StatisticsStore};

mod detection_cache;
mod diagnostics;
pub use diagnostics::{FailureReason, OperationMetrics};
