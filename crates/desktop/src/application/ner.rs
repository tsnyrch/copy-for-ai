use super::{
    OperationMetrics,
    detection_cache::DetectionCache,
    workflow::{PendingProcessing, WorkflowSnapshot},
};
use copy_for_ai_core::{Detector, Direction};
use copy_for_ai_ner::{DetectionPhase, DetectionProgress, Error, LocalNer, model_directory};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

struct NerState {
    model: Option<LocalNer>,
    cache: Option<DetectionCache>,
}

struct Mailbox {
    value: Option<(u64, DetectionProgress)>,
    last: Option<Instant>,
}

pub struct ProcessingReport {
    pub result: Result<PendingProcessing, Error>,
    pub metrics: OperationMetrics,
}

#[derive(Clone)]
pub struct NerService {
    state: Arc<Mutex<NerState>>,
    generation: Arc<AtomicU64>,
    cache_epoch: Arc<AtomicU64>,
    progress: Arc<Mutex<Mailbox>>,
    detector: Arc<Detector>,
}

impl NerService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NerState {
                model: None,
                cache: None,
            })),
            generation: Arc::new(AtomicU64::new(0)),
            cache_epoch: Arc::new(AtomicU64::new(0)),
            progress: Arc::new(Mutex::new(Mailbox {
                value: None,
                last: None,
            })),
            detector: Arc::new(Detector::new()),
        }
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cache_epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub fn forget(&self) {
        self.cancel();
        self.release_cache(self.cache_epoch.load(Ordering::SeqCst));
    }

    fn release_cache(&self, epoch: u64) {
        let state = self.state.clone();
        smol::spawn(async move {
            smol::unblock(move || {
                if let Ok(mut state) = state.lock()
                    && state
                        .cache
                        .as_ref()
                        .is_some_and(|cache| cache.invalidated_by(epoch))
                {
                    state.cache = None;
                }
            })
            .await;
        })
        .detach();
    }

    pub fn take_progress(&self) -> Option<DetectionProgress> {
        let (ticket, progress) = self.progress.try_lock().ok()?.value.take()?;
        (ticket == self.generation.load(Ordering::SeqCst)).then_some(progress)
    }

    pub fn process(
        &self,
        text: Zeroizing<String>,
        snapshot: WorkflowSnapshot,
    ) -> impl Future<Output = ProcessingReport> + use<> {
        let service = self.clone();
        let ticket = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let epoch = self.cache_epoch.load(Ordering::SeqCst);
        if let Ok(mut mailbox) = self.progress.try_lock() {
            mailbox.value = None;
            mailbox.last = None;
        }
        async move {
            smol::unblock(move || {
                let mut metrics = OperationMetrics::default();
                let result = service.run(text, snapshot, ticket, epoch, &mut metrics);
                ProcessingReport { result, metrics }
            })
            .await
        }
    }

    fn publish(&self, ticket: u64, value: DetectionProgress) {
        if self.generation.load(Ordering::SeqCst) != ticket {
            return;
        }
        if let Ok(mut mailbox) = self.progress.try_lock() {
            if mailbox
                .last
                .is_some_and(|last| last.elapsed() < Duration::from_millis(250))
            {
                return;
            }
            mailbox.last = Some(Instant::now());
            mailbox.value = Some((ticket, value));
        }
    }

    fn run(
        &self,
        text: Zeroizing<String>,
        snapshot: WorkflowSnapshot,
        ticket: u64,
        epoch: u64,
        metrics: &mut OperationMetrics,
    ) -> Result<PendingProcessing, Error> {
        let cancelled = || self.generation.load(Ordering::SeqCst) != ticket;
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let direction = snapshot.direction(&text).map_err(Error::Processing)?;
        let phase = |phase| {
            self.publish(
                ticket,
                DetectionProgress {
                    phase,
                    total: 0,
                    completed: 0,
                    reused: 0,
                    elapsed: Duration::ZERO,
                },
            )
        };
        if direction != Direction::Anonymize {
            let cutoff = self.cache_epoch.fetch_add(1, Ordering::SeqCst) + 1;
            self.release_cache(cutoff);
            phase(DetectionPhase::Core);
            let started = Instant::now();
            let result = snapshot
                .finish_cancellable(&self.detector, text, &[], &cancelled)
                .map_err(Error::Processing);
            metrics.core = started.elapsed();
            return if cancelled() {
                Err(Error::Cancelled)
            } else {
                result
            };
        }
        let mut state = self.state.lock().map_err(|_| Error::Load)?;
        if cancelled() {
            return Err(Error::Cancelled);
        }
        if state.model.is_none() {
            phase(DetectionPhase::Loading);
            state.cache = None;
            state.model = Some(LocalNer::load_configured(
                &model_directory(),
                Default::default(),
                cancelled,
                &mut metrics.ner,
            )?);
        }
        let model = state.model.as_ref().ok_or(Error::Load)?;
        let identity = model.identity();
        let hit = state
            .cache
            .as_ref()
            .is_some_and(|cache| cache.matches(&text, identity, epoch));
        let fresh = if hit {
            None
        } else {
            state.cache = None;
            Some(
                state
                    .model
                    .as_ref()
                    .ok_or(Error::Load)?
                    .detect_with_progress(&text, cancelled, &mut metrics.ner, |progress| {
                        self.publish(ticket, progress)
                    })?,
            )
        };
        let spans = if let Some(entities) = fresh.as_ref() {
            entities.as_slice()
        } else {
            let cache = state.cache.as_ref().ok_or(Error::Inference)?;
            metrics.ner = cache.metrics();
            metrics.cached_detection = true;
            cache.entities()
        };
        if cancelled() {
            return Err(Error::Cancelled);
        }
        phase(DetectionPhase::Core);
        let started = Instant::now();
        let result = snapshot
            .finish_cancellable(&self.detector, text, spans, &cancelled)
            .map_err(Error::Processing);
        metrics.core = started.elapsed();
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let pending = result?;
        if let Some(entities) = fresh
            && self.cache_epoch.load(Ordering::SeqCst) == epoch
        {
            state.cache = DetectionCache::new(
                pending.input(),
                identity,
                entities,
                metrics.ner.clone(),
                epoch,
            );
        }
        if cancelled() {
            state.cache = None;
            return Err(Error::Cancelled);
        }
        Ok(pending)
    }
}

#[cfg(test)]
#[path = "../../tests/application/ner.rs"]
mod tests;
