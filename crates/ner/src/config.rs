use std::time::Duration;

#[derive(Clone, Default)]
pub struct Metrics {
    pub verification: Duration,
    pub loading: Duration,
    pub windows: usize,
    pub inferred_windows: usize,
    pub reused_windows: usize,
    pub windowing: Duration,
    pub tokenization: Duration,
    pub preprocessing: Duration,
    pub inference: Duration,
    pub decoding: Duration,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModelVariant {
    Fp32,
    Int8,
    Q4,
}

impl ModelVariant {
    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Self::Fp32 => "model.onnx",
            Self::Int8 => "model-int8.onnx",
            Self::Q4 => "model-q4.onnx",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub token_budget: usize,
    pub batch_size: usize,
    pub threads: usize,
    pub variant: ModelVariant,
    pub coreml: bool,
    pub profile: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            token_budget: 256,
            batch_size: 1,
            threads: std::thread::available_parallelism().map_or(4, |count| count.get().min(8)),
            variant: ModelVariant::Fp32,
            coreml: false,
            profile: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DetectionIdentity {
    pub config: RuntimeConfig,
    pub model_digest: &'static str,
    pub tokenizer_digest: &'static str,
    pub threshold: u32,
}
