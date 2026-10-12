use super::ShieldView;
use crate::{
    application::{FailureReason, OperationStatus, PendingProcessing},
    platform,
};
#[cfg(test)]
use copy_for_ai_core::DetectedEntity;
use copy_for_ai_core::{Direction, Processed};
use gpui_kit::*;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
pub(super) enum Purpose {
    Preview,
    Clipboard(i64),
    Protected(platform::Snapshot),
}

impl ShieldView {
    pub(super) fn start_processing(
        &mut self,
        input: Zeroizing<String>,
        purpose: Purpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.quitting {
            return;
        }
        if self.busy {
            self.reject("Processing is in progress. Wait or cancel it.", cx);
            return;
        }
        let detected_direction = self.workflow.direction(&input);
        self.history
            .begin(detected_direction.as_ref().ok().copied(), input.len());
        let direction = match detected_direction {
            Ok(direction) => direction,
            Err(error) => {
                self.fail_processing(error.to_string(), cx);
                return;
            }
        };
        self.generation += 1;
        if matches!(purpose, Purpose::Preview) {
            self.output_direction = None;
            self.output
                .update(cx, |state, cx| state.set_value("", window, cx));
        }
        if input.trim().is_empty() {
            self.fail_processing("Input is empty.", cx);
            return;
        }
        self.busy = true;
        self.paste_started = None;
        self.notice(
            if direction == Direction::Anonymize {
                "Loading / local multilingual detection…"
            } else {
                "Exact local restoration…"
            },
            cx,
        );
        let generation = self.generation;
        let detection = self.ner.process(input, self.workflow.snapshot());
        cx.spawn_in(window, async move |view, cx| {
            let report = detection.await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.generation == generation && this.busy {
                    this.history.set_metrics(report.metrics);
                }
                this.finish_detection(generation, report.result, purpose, window, cx);
            });
        })
        .detach();
    }

    pub(super) fn finish_detection(
        &mut self,
        generation: u64,
        result: Result<PendingProcessing, copy_for_ai_ner::Error>,
        purpose: Purpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.generation != generation {
            return;
        }
        self.busy = false;
        match result {
            Ok(pending) => {
                if matches!(purpose, Purpose::Preview)
                    && self.input.read(cx).value().as_ref() != pending.input()
                {
                    self.fail_processing_reason(
                        FailureReason::InputChanged,
                        FailureReason::InputChanged.message(),
                        cx,
                    );
                    return;
                }
                if let Some(reason) = self.destination_error(purpose) {
                    self.fail_processing_reason(reason, reason.message(), cx);
                    return;
                }
                if !self.workflow.can_commit(&pending) {
                    self.fail_processing_reason(
                        FailureReason::StaleWorkflow,
                        FailureReason::StaleWorkflow.message(),
                        cx,
                    );
                    return;
                }
                if let Purpose::Protected(snapshot) = purpose
                    && pending.output().direction != Direction::Unresolved
                {
                    self.last_count = pending.output().replacements;
                    self.finish_paste(pending, snapshot, window, cx);
                    return;
                }
                match self.workflow.commit(pending) {
                    Ok(output) => self.publish_result(output, purpose, window, cx),
                    Err(error) => self.fail_processing(error.to_string(), cx),
                }
            }
            Err(error) => {
                if matches!(error, copy_for_ai_ner::Error::Cancelled) {
                    self.history.finish(OperationStatus::Cancelled, 0);
                    self.notice("Operation cancelled. Nothing was pasted.", cx);
                } else {
                    let reason = if matches!(error, copy_for_ai_ner::Error::Processing(_)) {
                        FailureReason::ProcessingFailure
                    } else {
                        FailureReason::ModelFailure
                    };
                    self.fail_processing_reason(reason, error.to_string(), cx);
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn complete_processing(
        &mut self,
        input: &str,
        entities: &[DetectedEntity],
        purpose: Purpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(purpose, Purpose::Preview) && self.input.read(cx).value().as_ref() != input {
            self.fail_processing_reason(
                FailureReason::InputChanged,
                FailureReason::InputChanged.message(),
                cx,
            );
            return;
        }
        if !self.valid_destination(purpose) {
            self.fail_processing(
                "Target, clipboard or permission changed. Nothing was pasted.",
                cx,
            );
            return;
        }
        match self.workflow.process(input, entities) {
            Ok(output) => self.publish_result(output, purpose, window, cx),
            Err(error) => self.fail_processing(error.to_string(), cx),
        }
    }

    fn publish_result(
        &mut self,
        output: Processed,
        purpose: Purpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.last_count = output.replacements;
        if matches!(purpose, Purpose::Preview) {
            self.output_direction = Some(output.direction);
        }
        let message = match output.direction {
            Direction::Anonymize => format!("Anonymized {} occurrences.", output.replacements),
            Direction::Restore => format!(
                "Restored {} occurrences; unknown markers: {}.",
                output.replacements,
                output.unknown_tokens.len()
            ),
            Direction::Unresolved => {
                "Markers have no local mappings. Text preserved; copy and paste stopped.".into()
            }
        };
        if output.direction == Direction::Unresolved {
            if matches!(purpose, Purpose::Preview) {
                self.output
                    .update(cx, |state, cx| state.set_value(output.text, window, cx));
            }
            self.fail_processing(message, cx);
            return;
        }
        match purpose {
            Purpose::Preview => {
                self.output
                    .update(cx, |state, cx| state.set_value(output.text, window, cx));
                self.history
                    .finish(OperationStatus::Completed, output.replacements);
                if output.unknown_tokens.is_empty() {
                    self.notice(message, cx);
                } else {
                    self.fail_processing(message, cx);
                }
            }
            Purpose::Clipboard(_) => {
                cx.write_to_clipboard(ClipboardItem::new_string(output.text));
                self.history
                    .finish(OperationStatus::Completed, output.replacements);
                if output.unknown_tokens.is_empty() {
                    self.notice(message, cx);
                } else {
                    self.fail_processing(message, cx);
                }
            }
            Purpose::Protected(_) => {
                self.fail_processing("Paste was not prepared. Nothing was pasted.", cx)
            }
        }
    }

    fn destination_error(&self, purpose: Purpose) -> Option<FailureReason> {
        match purpose {
            Purpose::Preview => None,
            Purpose::Clipboard(version) => (self.native.clipboard_version() != version)
                .then_some(FailureReason::ClipboardChanged),
            Purpose::Protected(snapshot) => {
                if self.native.target() != Some(snapshot.target) {
                    Some(FailureReason::TargetChanged)
                } else if self.native.clipboard_version() != snapshot.clipboard_version {
                    Some(FailureReason::ClipboardChanged)
                } else if !self.native.accessibility_trusted() {
                    Some(FailureReason::PermissionMissing)
                } else {
                    None
                }
            }
        }
    }

    #[cfg(test)]
    fn valid_destination(&self, purpose: Purpose) -> bool {
        self.destination_error(purpose).is_none()
    }

    fn finish_paste(
        &mut self,
        pending: PendingProcessing,
        snapshot: platform::Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.paste_started = Some(Instant::now());
        let generation = self.generation;
        self.notice("Text ready. Release the keys…", cx);
        let executor = cx.background_executor().clone();
        let native = self.native.clone();
        cx.spawn_in(window, async move |view, cx| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !native.modifiers_released() && Instant::now() < deadline {
                executor.timer(Duration::from_millis(15)).await;
            }
            let _ = view.update_in(cx, |this, _, cx| {
                this.deliver_paste(generation, pending, snapshot, cx);
            });
        })
        .detach();
    }

    pub(super) fn deliver_paste(
        &mut self,
        generation: u64,
        pending: PendingProcessing,
        snapshot: platform::Snapshot,
        cx: &mut Context<Self>,
    ) {
        if self.generation != generation || !self.busy {
            return;
        }
        self.busy = false;
        if let Some(started) = self.paste_started.take() {
            self.history.set_paste_wait(started.elapsed());
        }
        let failure = self
            .destination_error(Purpose::Protected(snapshot))
            .or_else(|| {
                (!self.native.modifiers_released()).then_some(FailureReason::ModifiersPressed)
            })
            .or_else(|| {
                (!self.workflow.can_commit(&pending)).then_some(FailureReason::StaleWorkflow)
            });
        if let Some(reason) = failure {
            self.fail_processing_reason(reason, reason.message(), cx);
            return;
        }
        if pending.output().direction == Direction::Unresolved {
            self.fail_processing("Markers have no local mappings. Nothing was pasted.", cx);
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(pending.output().text.clone()));
        let written_version = self.native.clipboard_version();
        if let Err(error) = self.native.paste(platform::Snapshot {
            clipboard_version: written_version,
            ..snapshot
        }) {
            if self.native.clipboard_version() == written_version {
                cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            }
            self.fail_processing_reason(FailureReason::NativeFailure, error, cx);
            return;
        }
        match self.workflow.commit(pending) {
            Ok(output) => {
                self.history
                    .finish(OperationStatus::Completed, output.replacements);
                let message = format!(
                    "Paste shortcut sent · {} replacements · {} unknown markers.",
                    output.replacements,
                    output.unknown_tokens.len()
                );
                if output.unknown_tokens.is_empty() {
                    self.notice(message, cx);
                } else {
                    self.fail_processing(message, cx);
                }
            }
            Err(error) => self.fail_processing(error.to_string(), cx),
        }
    }

    pub(super) fn cancel_processing(&mut self, cx: &mut Context<Self>) {
        if let Some(started) = self.paste_started.take() {
            self.history.set_paste_wait(started.elapsed());
        }
        self.history.finish(OperationStatus::Cancelled, 0);
        self.generation += 1;
        self.ner.cancel();
        self.busy = false;
        self.notice("Operation cancelled. Clipboard unchanged.", cx);
    }
}
