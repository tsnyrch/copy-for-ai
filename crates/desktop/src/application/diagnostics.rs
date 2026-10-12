use copy_for_ai_ner::Metrics;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct OperationMetrics {
    pub ner: Metrics,
    pub core: Duration,
    pub paste_wait: Duration,
    pub cached_detection: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    InputChanged,
    TargetChanged,
    ClipboardChanged,
    PermissionMissing,
    ModifiersPressed,
    StaleWorkflow,
    ModelFailure,
    NativeFailure,
    ProcessingFailure,
}

impl FailureReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::InputChanged => "Input changed during processing.",
            Self::TargetChanged => "Target changed. Nothing was pasted.",
            Self::ClipboardChanged => "Clipboard changed. Nothing was written or pasted.",
            Self::PermissionMissing => "Accessibility permission is missing. Nothing was pasted.",
            Self::ModifiersPressed => "Keys were not released. Nothing was pasted.",
            Self::StaleWorkflow => "Vault or rules changed. Process the text again.",
            Self::ModelFailure => "Model detection failed. Nothing was pasted.",
            Self::NativeFailure => "System paste failed.",
            Self::ProcessingFailure => "Processing failed.",
        }
    }
}
