use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetectionPhase {
    Loading,
    Preparing,
    Detecting,
    Core,
}

#[derive(Clone, Copy)]
pub struct DetectionProgress {
    pub phase: DetectionPhase,
    pub total: usize,
    pub completed: usize,
    pub reused: usize,
    pub elapsed: Duration,
}

pub(crate) struct Reporter<F> {
    callback: F,
    started: Instant,
    published: Option<Instant>,
}

impl<F: FnMut(DetectionProgress)> Reporter<F> {
    pub(crate) fn new(callback: F) -> Self {
        Self {
            callback,
            started: Instant::now(),
            published: None,
        }
    }

    pub(crate) fn publish(
        &mut self,
        phase: DetectionPhase,
        total: usize,
        completed: usize,
        reused: usize,
    ) {
        let now = Instant::now();
        if self
            .published
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(250))
        {
            return;
        }
        self.published = Some(now);
        (self.callback)(DetectionProgress {
            phase,
            total,
            completed,
            reused,
            elapsed: self.started.elapsed(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_callbacks_are_coalesced_without_losing_window_counts() {
        let mut calls = Vec::new();
        let mut reporter = Reporter::new(|progress| calls.push(progress));
        reporter.publish(DetectionPhase::Preparing, 0, 0, 0);
        reporter.publish(DetectionPhase::Detecting, 10, 1, 0);
        reporter.published = Some(Instant::now() - Duration::from_millis(251));
        reporter.publish(DetectionPhase::Detecting, 10, 4, 2);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].total, 10);
        assert_eq!(calls[1].completed, 4);
        assert_eq!(calls[1].reused, 2);
    }
}
