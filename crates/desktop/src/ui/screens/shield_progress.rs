use super::ShieldView;
use copy_for_ai_ner::DetectionPhase;
use gpui_kit::*;

impl ShieldView {
    pub(super) fn poll_processing_progress(&mut self, cx: &mut Context<Self>) {
        if !self.busy || self.paste_started.is_some() {
            return;
        }
        let Some(progress) = self.ner.take_progress() else {
            return;
        };
        let message = match progress.phase {
            DetectionPhase::Loading => "Verifying and loading the local model…".to_owned(),
            DetectionPhase::Preparing => "Preparing text and token chunks…".to_owned(),
            DetectionPhase::Core => "Building the result…".to_owned(),
            DetectionPhase::Detecting => {
                let mut message = format!(
                    "Processed {} / {} chunks · {} reused",
                    progress.completed, progress.total, progress.reused
                );
                if progress.completed >= 3 && progress.completed < progress.total {
                    let remaining = progress.elapsed.as_secs_f64()
                        * (progress.total - progress.completed) as f64
                        / progress.completed as f64;
                    message.push_str(&format!(" · estimated {:.0} s remaining", remaining));
                }
                message
            }
        };
        self.notice(message, cx);
    }
}
