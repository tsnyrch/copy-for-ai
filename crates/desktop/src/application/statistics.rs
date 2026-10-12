use super::history::{Operation, OperationStatus};
use copy_for_ai_core::Direction;
use std::{
    fs,
    io::{ErrorKind, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::UNIX_EPOCH,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct LifetimeStatistics {
    pub completed: u64,
    pub cancelled: u64,
    pub failed: u64,
    pub anonymize: u64,
    pub restore: u64,
    pub unresolved: u64,
    pub unknown_direction: u64,
    pub input_bytes: u64,
    pub replacements: u64,
    pub duration_micros: u64,
    pub first_operation: u64,
}

impl LifetimeStatistics {
    pub fn operations(self) -> u64 {
        self.completed
            .saturating_add(self.cancelled)
            .saturating_add(self.failed)
    }
    pub fn average_millis(self) -> u64 {
        self.duration_micros
            .checked_div(self.operations())
            .unwrap_or(0)
            / 1_000
    }

    pub(super) fn record(&mut self, operation: &Operation) {
        if self.first_operation == 0 {
            self.first_operation = operation
                .time
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }
        let count = match operation.status {
            OperationStatus::Completed => &mut self.completed,
            OperationStatus::Cancelled => &mut self.cancelled,
            OperationStatus::Failed => &mut self.failed,
        };
        *count = count.saturating_add(1);
        let direction = match operation.direction {
            Some(Direction::Anonymize) => &mut self.anonymize,
            Some(Direction::Restore) => &mut self.restore,
            Some(Direction::Unresolved) => &mut self.unresolved,
            None => &mut self.unknown_direction,
        };
        *direction = direction.saturating_add(1);
        self.input_bytes = self
            .input_bytes
            .saturating_add(operation.input_bytes as u64);
        self.replacements = self
            .replacements
            .saturating_add(operation.replacements as u64);
        self.duration_micros = self
            .duration_micros
            .saturating_add(operation.duration.as_micros().min(u128::from(u64::MAX)) as u64);
    }

    fn encode(self) -> String {
        format!(
            "version=1\ncompleted={}\ncancelled={}\nfailed={}\nanonymize={}\nrestore={}\nunresolved={}\nunknown_direction={}\ninput_bytes={}\nreplacements={}\nduration_micros={}\nfirst_operation={}\n",
            self.completed,
            self.cancelled,
            self.failed,
            self.anonymize,
            self.restore,
            self.unresolved,
            self.unknown_direction,
            self.input_bytes,
            self.replacements,
            self.duration_micros,
            self.first_operation
        )
    }

    fn decode(text: &str) -> Result<Self, &'static str> {
        let mut lines = text.lines();
        if lines.next() != Some("version=1") {
            return Err("Unsupported lifetime statistics version.");
        }
        let mut values = [0_u64; 11];
        for (value, key) in values.iter_mut().zip([
            "completed",
            "cancelled",
            "failed",
            "anonymize",
            "restore",
            "unresolved",
            "unknown_direction",
            "input_bytes",
            "replacements",
            "duration_micros",
            "first_operation",
        ]) {
            *value = lines
                .next()
                .and_then(|line| line.strip_prefix(key))
                .and_then(|value| value.strip_prefix('='))
                .and_then(|value| value.parse().ok())
                .ok_or("Invalid lifetime statistics format.")?;
        }
        if lines.next().is_some() {
            return Err("Lifetime statistics contain unknown fields.");
        }
        let [
            completed,
            cancelled,
            failed,
            anonymize,
            restore,
            unresolved,
            unknown_direction,
            input_bytes,
            replacements,
            duration_micros,
            first_operation,
        ] = values;
        let statuses = completed
            .checked_add(cancelled)
            .and_then(|sum| sum.checked_add(failed));
        let directions = anonymize
            .checked_add(restore)
            .and_then(|sum| sum.checked_add(unresolved))
            .and_then(|sum| sum.checked_add(unknown_direction));
        if statuses.is_none() || statuses != directions {
            return Err("Lifetime statistics counts are inconsistent.");
        }
        Ok(Self {
            completed,
            cancelled,
            failed,
            anonymize,
            restore,
            unresolved,
            unknown_direction,
            input_bytes,
            replacements,
            duration_micros,
            first_operation,
        })
    }
}

#[derive(Clone)]
pub struct StatisticsStore {
    path: Option<PathBuf>,
    writer: Arc<smol::lock::Mutex<()>>,
    scheduled: Arc<AtomicU64>,
}

impl StatisticsStore {
    pub fn temporary() -> Self {
        Self {
            path: None,
            writer: Arc::default(),
            scheduled: Arc::default(),
        }
    }
    pub fn is_persistent(&self) -> bool {
        self.path.is_some()
    }

    pub fn resettable() -> Result<Self, &'static str> {
        Ok(Self {
            path: Some(super::settings::data_directory()?.join("statistics.conf")),
            writer: Arc::default(),
            scheduled: Arc::default(),
        })
    }

    pub fn load() -> Result<(Self, LifetimeStatistics), &'static str> {
        Self::load_path(super::settings::data_directory()?.join("statistics.conf"))
    }

    fn load_path(path: PathBuf) -> Result<(Self, LifetimeStatistics), &'static str> {
        let totals = match fs::File::open(&path) {
            Ok(file) => {
                let mut text = String::new();
                file.take(1_025)
                    .read_to_string(&mut text)
                    .map_err(|_| "Could not read lifetime statistics.")?;
                if text.len() > 1_024 {
                    return Err("Lifetime statistics file is too large.");
                }
                LifetimeStatistics::decode(&text)?
            }
            Err(error) if error.kind() == ErrorKind::NotFound => LifetimeStatistics::default(),
            Err(_) => return Err("Could not read lifetime statistics."),
        };
        Ok((
            Self {
                path: Some(path),
                writer: Arc::default(),
                scheduled: Arc::default(),
            },
            totals,
        ))
    }

    /// Orders snapshots at submission, so delayed saves cannot overwrite a newer reset.
    pub fn save(
        &self,
        totals: LifetimeStatistics,
    ) -> impl Future<Output = Result<(), &'static str>> + Send + 'static {
        let store = self.clone();
        let revision = store.scheduled.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            let _writer = store.writer.lock().await;
            if revision != store.scheduled.load(Ordering::SeqCst) {
                return Ok(());
            }
            smol::unblock(move || {
                if let Some(path) = store.path {
                    let parent = path.parent().ok_or("Could not save lifetime statistics.")?;
                    fs::create_dir_all(parent)
                        .map_err(|_| "Could not create the statistics directory.")?;
                    let temporary = path.with_extension("conf.tmp");
                    fs::write(&temporary, totals.encode())
                        .map_err(|_| "Could not save lifetime statistics.")?;
                    fs::rename(temporary, path)
                        .map_err(|_| "Could not finish saving lifetime statistics.")?;
                }
                Ok(())
            })
            .await
        }
    }
}

#[cfg(test)]
#[path = "../../tests/application/statistics.rs"]
mod tests;
