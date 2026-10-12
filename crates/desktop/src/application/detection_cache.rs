use copy_for_ai_core::DetectedEntity;
use copy_for_ai_ner::{DetectionIdentity, Metrics};
use zeroize::Zeroizing;

const MAX_CACHE_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct DetectionCache {
    input: Zeroizing<String>,
    identity: DetectionIdentity,
    entities: Vec<DetectedEntity>,
    metrics: Metrics,
    epoch: u64,
}

impl DetectionCache {
    pub(super) fn new(
        input: &str,
        identity: DetectionIdentity,
        entities: Vec<DetectedEntity>,
        metrics: Metrics,
        epoch: u64,
    ) -> Option<Self> {
        let input = Zeroizing::new(input.to_owned());
        let bytes = input
            .capacity()
            .checked_add(
                entities
                    .capacity()
                    .checked_mul(std::mem::size_of::<DetectedEntity>())?,
            )?
            .checked_add(std::mem::size_of::<Self>())?;
        if bytes > MAX_CACHE_BYTES {
            return None;
        }
        Some(Self {
            input,
            identity,
            entities,
            metrics,
            epoch,
        })
    }

    pub(super) fn matches(&self, input: &str, identity: DetectionIdentity, epoch: u64) -> bool {
        self.epoch == epoch && self.identity == identity && self.input.as_str() == input
    }

    pub(super) fn invalidated_by(&self, epoch: u64) -> bool {
        self.epoch < epoch
    }

    pub(super) fn entities(&self) -> &[DetectedEntity] {
        &self.entities
    }

    pub(super) fn metrics(&self) -> Metrics {
        Metrics {
            windows: self.metrics.windows,
            reused_windows: self.metrics.windows,
            ..Default::default()
        }
    }
}

#[cfg(test)]
#[path = "../../tests/application/detection_cache.rs"]
mod tests;
