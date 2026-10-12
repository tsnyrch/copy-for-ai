mod assets;
mod chunks;
mod config;
pub use config::{DetectionIdentity, Metrics, ModelVariant, RuntimeConfig};
mod pipeline;
mod progress;
pub use progress::{DetectionPhase, DetectionProgress};

use copy_for_ai_core::{DetectedEntity, EntityKind, MAX_INPUT_BYTES};
use gliner::model::params::Parameters;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use thiserror::Error;

pub use assets::model_directory;

const LABELS: &[&str] = &[
    "person",
    "first name",
    "organization",
    "company",
    "location",
    "city",
    "address",
    "event",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("Local multilingual model is missing. Run python3 scripts/install-model.py.")]
    Missing,
    #[error("Model files do not match the verified revision. Reinstall the model.")]
    InvalidAssets,
    #[error("Could not load the local model. Operation stopped.")]
    Load,
    #[error("Local detection failed. Operation stopped.")]
    Inference,
    #[error("Operation cancelled.")]
    Cancelled,
    #[error("Detection timed out. Nothing was pasted.")]
    Timeout,
    #[error("Text exceeds the 1 MiB limit.")]
    InputTooLarge,
    #[error("Detection exceeds memory capacity. Operation stopped.")]
    Capacity,
    #[error(transparent)]
    Processing(copy_for_ai_core::Error),
}

pub struct LocalNer {
    model: ort::session::Session,
    pipeline: pipeline::LocalPipeline,
    parameters: Parameters,
    config: RuntimeConfig,
}

impl LocalNer {
    pub fn load(directory: &Path) -> Result<Self, Error> {
        Self::load_measured(directory, &mut Metrics::default())
    }

    pub fn load_measured(directory: &Path, metrics: &mut Metrics) -> Result<Self, Error> {
        Self::load_configured(directory, RuntimeConfig::default(), || false, metrics)
    }

    pub fn load_configured(
        directory: &Path,
        config: RuntimeConfig,
        cancelled: impl Fn() -> bool,
        metrics: &mut Metrics,
    ) -> Result<Self, Error> {
        Self::load_with_backend(directory, config, Ok, cancelled, metrics)
    }

    pub fn load_with_backend(
        directory: &Path,
        config: RuntimeConfig,
        configure: impl FnOnce(
            ort::session::builder::SessionBuilder,
        ) -> ort::Result<ort::session::builder::SessionBuilder>,
        cancelled: impl Fn() -> bool,
        metrics: &mut Metrics,
    ) -> Result<Self, Error> {
        if !(64..=480).contains(&config.token_budget)
            || !(1..=2).contains(&config.batch_size)
            || !(1..=8).contains(&config.threads)
        {
            return Err(Error::Load);
        }
        let start = Instant::now();
        assets::verify(directory, config.variant, &cancelled)?;
        metrics.verification = start.elapsed();
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let start = Instant::now();
        let pipeline = pipeline::LocalPipeline::new(&directory.join("tokenizer.json"))?;
        let mut builder = ort::session::Session::builder()
            .and_then(|b| b.with_intra_threads(config.threads))
            .and_then(|b| {
                b.with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            })
            .map_err(|_| Error::Load)?;
        if config.coreml {
            builder = builder
                .with_execution_providers([
                    ort::execution_providers::CoreMLExecutionProvider::default()
                        .build()
                        .error_on_failure(),
                ])
                .map_err(|_| Error::Load)?;
        }
        builder = configure(builder).map_err(|_| Error::Load)?;
        if config.profile {
            builder = builder
                .with_profiling(
                    std::env::temp_dir().join(format!("copy-for-ai-onnx-{}", std::process::id())),
                )
                .map_err(|_| Error::Load)?;
        }
        let model = builder
            .commit_from_file(directory.join(config.variant.file_name()))
            .map_err(|_| Error::Load)?;
        metrics.loading = start.elapsed();
        if cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(Self {
            model,
            pipeline,
            parameters: Parameters::default()
                .with_threshold(0.4)
                .with_max_length(None),
            config,
        })
    }

    pub fn finish_profiling(&self) -> Result<String, Error> {
        self.model.end_profiling().map_err(|_| Error::Inference)
    }

    pub fn detect(
        &self,
        text: &str,
        cancelled: impl Fn() -> bool,
    ) -> Result<Vec<DetectedEntity>, Error> {
        self.detect_measured(text, cancelled, &mut Metrics::default())
    }

    pub fn detect_measured(
        &self,
        text: &str,
        cancelled: impl Fn() -> bool,
        metrics: &mut Metrics,
    ) -> Result<Vec<DetectedEntity>, Error> {
        self.detect_with_progress(text, cancelled, metrics, |_| {})
    }

    pub fn identity(&self) -> DetectionIdentity {
        DetectionIdentity {
            config: self.config,
            model_digest: assets::model_spec(self.config.variant).2,
            tokenizer_digest: assets::TOKENIZER_DIGEST,
            threshold: self.parameters.threshold.to_bits(),
        }
    }

    pub fn detect_with_progress(
        &self,
        text: &str,
        cancelled: impl Fn() -> bool,
        metrics: &mut Metrics,
        progress: impl FnMut(DetectionProgress),
    ) -> Result<Vec<DetectedEntity>, Error> {
        if text.len() > MAX_INPUT_BYTES {
            return Err(Error::InputTooLarge);
        }
        let mut reporter = progress::Reporter::new(progress);
        reporter.publish(DetectionPhase::Preparing, 0, 0, 0);
        let deadline = Instant::now() + Duration::from_secs(15 * 60);
        let check = || {
            if cancelled() {
                Err(Error::Cancelled)
            } else if Instant::now() >= deadline {
                Err(Error::Timeout)
            } else {
                Ok(())
            }
        };
        check()?;
        let start = Instant::now();
        let mut windows = chunks::Windows::prepare(
            text,
            &self.pipeline.segmenter,
            &self.pipeline.tokenizer,
            self.config.token_budget - self.pipeline.prompt_length(),
            &cancelled,
            deadline,
            &mut metrics.tokenization,
        )?;
        let total = windows.total();
        metrics.windowing = start.elapsed().saturating_sub(metrics.tokenization);
        let mut completed = 0;
        let mut entities = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut cache: std::collections::HashMap<&str, Vec<(usize, usize, EntityKind)>> =
            std::collections::HashMap::new();
        loop {
            reporter.publish(
                DetectionPhase::Detecting,
                total,
                completed,
                metrics.reused_windows,
            );
            check()?;
            let requested: Vec<_> = (0..self.config.batch_size)
                .filter_map(|_| windows.next())
                .collect();
            if requested.is_empty() {
                break;
            }
            metrics.windows += requested.len();
            let requested_count = requested.len();
            let mut ranges = Vec::new();
            for range in requested {
                let bytes = pipeline::chunk_range(text, &windows.words, &range)?;
                if let Some(spans) = cache.get(&text[bytes.clone()]) {
                    metrics.reused_windows += 1;
                    for &(from, to, kind) in spans {
                        if seen.insert((bytes.start + from, bytes.start + to, kind)) {
                            if entities.len() == 1024 * 1024 {
                                return Err(Error::Capacity);
                            }
                            entities.push(DetectedEntity {
                                range: bytes.start + from..bytes.start + to,
                                kind,
                            });
                        }
                    }
                } else {
                    ranges.push(range);
                }
            }
            if ranges.is_empty() {
                completed += requested_count;
                continue;
            }
            metrics.inferred_windows += ranges.len();
            let start = Instant::now();
            let (input, offsets) =
                self.pipeline
                    .encode(text, &windows.words, &ranges, self.parameters.max_width)?;
            metrics.preprocessing += start.elapsed();
            check()?;
            let start = Instant::now();
            let output = self
                .model
                .run(input.tensors)
                .map_err(|_| Error::Inference)?;
            metrics.inference += start.elapsed();
            check()?;
            let start = Instant::now();
            let output = self
                .pipeline
                .decode(output, input.context, &self.parameters)?;
            metrics.decoding += start.elapsed();
            if output.spans.len() != ranges.len() {
                return Err(Error::Inference);
            }
            for ((spans, offset), range) in output.spans.into_iter().zip(offsets).zip(ranges) {
                let mut cached = Vec::new();
                for span in spans {
                    let (from, to) = span.offsets();
                    if from >= to || text.get(offset + from..offset + to) != Some(span.text()) {
                        return Err(Error::Inference);
                    }
                    let kind = match span.class() {
                        "person" | "first name" => EntityKind::Person,
                        "organization" | "company" => EntityKind::Organization,
                        "location" | "city" => EntityKind::Location,
                        "address" => EntityKind::Address,
                        "event" => EntityKind::Event,
                        _ => return Err(Error::Inference),
                    };
                    cached.push((from, to, kind));
                    if seen.insert((offset + from, offset + to, kind)) {
                        if entities.len() == 1024 * 1024 {
                            return Err(Error::Capacity);
                        }
                        entities.push(DetectedEntity {
                            range: offset + from..offset + to,
                            kind,
                        });
                    }
                }
                if cache.len() < 64 && cached.len() <= 4096 {
                    let bytes = pipeline::chunk_range(text, &windows.words, &range)?;
                    cache.insert(&text[bytes], cached);
                }
            }
            completed += requested_count;
        }
        check()?;
        Ok(entities)
    }
}
