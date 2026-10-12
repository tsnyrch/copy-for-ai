use super::{FailureReason, OperationMetrics};
use copy_for_ai_core::Direction;
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    Completed,
    Cancelled,
    Failed,
}

pub struct Operation {
    pub time: SystemTime,
    pub direction: Option<Direction>,
    pub status: OperationStatus,
    pub duration: Duration,
    pub input_bytes: usize,
    pub replacements: usize,
    pub metrics: Option<OperationMetrics>,
    pub failure: Option<FailureReason>,
}

pub struct HistorySummary {
    pub completed: usize,
    pub cancelled: usize,
    pub failed: usize,
    pub replacements: usize,
}

struct ActiveOperation {
    metrics: Option<OperationMetrics>,
    failure: Option<FailureReason>,
    time: SystemTime,
    started: Instant,
    direction: Option<Direction>,
    input_bytes: usize,
}

/// Only scalar metadata is retained; document content never enters history.
pub struct OperationHistory {
    entries: VecDeque<Operation>,
    active: Option<ActiveOperation>,
    lifetime: super::LifetimeStatistics,
    revision: u64,
}

impl OperationHistory {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            active: None,
            lifetime: super::LifetimeStatistics::default(),
            revision: 0,
        }
    }

    pub fn begin(&mut self, direction: Option<Direction>, input_bytes: usize) {
        self.finish(OperationStatus::Cancelled, 0);
        self.active = Some(ActiveOperation {
            metrics: None,
            failure: None,
            time: SystemTime::now(),
            started: Instant::now(),
            direction,
            input_bytes,
        });
    }

    pub fn finish(&mut self, status: OperationStatus, replacements: usize) {
        let Some(active) = self.active.take() else {
            return;
        };
        if self.entries.len() == 100 {
            self.entries.pop_front();
        }
        let operation = Operation {
            time: active.time,
            direction: active.direction,
            status,
            duration: active.started.elapsed(),
            input_bytes: active.input_bytes,
            replacements,
            metrics: active.metrics,
            failure: active.failure,
        };
        self.lifetime.record(&operation);
        self.revision += 1;
        self.entries.push_back(operation);
    }

    pub fn set_metrics(&mut self, metrics: OperationMetrics) {
        if let Some(active) = self.active.as_mut() {
            active.metrics = Some(metrics);
        }
    }

    pub fn set_failure(&mut self, failure: FailureReason) {
        if let Some(active) = self.active.as_mut() {
            active.failure = Some(failure);
        }
    }

    pub fn set_paste_wait(&mut self, duration: Duration) {
        if let Some(active) = self.active.as_mut()
            && let Some(metrics) = active.metrics.as_mut()
        {
            metrics.paste_wait = duration;
        }
    }

    pub fn with_lifetime(lifetime: super::LifetimeStatistics) -> Self {
        Self {
            lifetime,
            ..Self::new()
        }
    }
    pub fn lifetime(&self) -> super::LifetimeStatistics {
        self.lifetime
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn entries(&self) -> &VecDeque<Operation> {
        &self.entries
    }

    pub fn summary(&self) -> HistorySummary {
        let completed = self
            .entries
            .iter()
            .filter(|entry| entry.status == OperationStatus::Completed)
            .count();
        let cancelled = self
            .entries
            .iter()
            .filter(|entry| entry.status == OperationStatus::Cancelled)
            .count();
        HistorySummary {
            completed,
            cancelled,
            failed: self.entries.len() - completed - cancelled,
            replacements: self.entries.iter().map(|entry| entry.replacements).sum(),
        }
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.entries.clear();
        self.lifetime = super::LifetimeStatistics::default();
        self.revision += 1;
    }
}

#[cfg(test)]
#[path = "../../tests/application/history.rs"]
mod tests;
