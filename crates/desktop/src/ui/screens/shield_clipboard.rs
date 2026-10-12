use super::ShieldView;
use crate::application::FailureReason;
use crate::{application::Shortcuts, platform};
use copy_for_ai_core::MAX_OUTPUT_BYTES;
use gpui_kit::*;
use zeroize::Zeroizing;

impl ShieldView {
    pub(super) fn read_clipboard(cx: &App) -> Result<Zeroizing<String>, &'static str> {
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .ok_or(
                "Clipboard contains no text. Images and files are not supported in this version.",
            )?;
        if text.len() > MAX_OUTPUT_BYTES {
            return Err("Clipboard exceeds the 32 MiB limit. Operation stopped.");
        }
        if text.trim().is_empty() {
            return Err("Clipboard is empty.");
        }
        Ok(Zeroizing::new(text))
    }

    pub(super) fn toggle_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.history
            .finish(crate::application::OperationStatus::Cancelled, 0);
        self.generation += 1;
        self.busy = false;
        self.ner.cancel();
        if self.shortcuts.take().is_some() {
            self.notice("Global shortcuts disabled.", cx);
            return;
        }
        if !platform::available() {
            self.fail(
                "Global paste is available only on macOS in this version. Use preview and copy.",
                cx,
            );
            return;
        }
        match Shortcuts::register() {
            Ok(shortcuts) => {
                self.shortcuts = Some(shortcuts);
                if self.native.accessibility_trusted() {
                    self.notice("Shortcuts active: Cmd+Shift+V anonymizes / restores and pastes; Cmd+Shift+R restores the clipboard only.", cx);
                } else {
                    self.fail("Shortcuts registered. To paste, allow the app in System Settings → Privacy & Security → Accessibility.", cx);
                }
            }
            Err(error) => self.fail(error, cx),
        }
    }

    pub(super) fn restore_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            self.reject("Processing is in progress. Wait or cancel it.", cx);
            return;
        }
        let version = self.native.clipboard_version();
        match Self::read_clipboard(cx) {
            Ok(input) => {
                if self.workflow.direction(&input) == Ok(copy_for_ai_core::Direction::Anonymize) {
                    self.history
                        .begin(Some(copy_for_ai_core::Direction::Restore), input.len());
                    self.fail_processing(
                        "Clipboard contains no markers to restore. Nothing changed.",
                        cx,
                    );
                    return;
                }
                self.start_processing(
                    input,
                    super::processing::Purpose::Clipboard(version),
                    window,
                    cx,
                );
            }
            Err(error) => {
                self.history.begin(None, 0);
                self.fail_processing(error, cx);
            }
        }
    }

    pub(super) fn protected_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let version = self.native.clipboard_version();
        let input = match Self::read_clipboard(cx) {
            Ok(text) => text,
            Err(error) => {
                self.history.begin(None, 0);
                self.fail_processing(error, cx);
                return;
            }
        };
        if self.workflow.direction(&input) == Ok(copy_for_ai_core::Direction::Unresolved) {
            self.history
                .begin(Some(copy_for_ai_core::Direction::Unresolved), input.len());
            self.fail_processing("Markers have no local mappings. Nothing was pasted.", cx);
            return;
        }
        if !self.native.accessibility_trusted() {
            self.history
                .begin(self.workflow.direction(&input).ok(), input.len());
            self.fail_processing_reason(FailureReason::PermissionMissing, "Protected paste requires Accessibility permission. Preview and restoration work without it.", cx);
            return;
        }
        let Some(snapshot) = self.native.snapshot() else {
            self.history
                .begin(self.workflow.direction(&input).ok(), input.len());
            self.fail_processing_reason(
                FailureReason::TargetChanged,
                "Could not identify the target app for paste.",
                cx,
            );
            return;
        };
        if snapshot.clipboard_version != version {
            self.history
                .begin(self.workflow.direction(&input).ok(), input.len());
            self.fail_processing_reason(
                FailureReason::ClipboardChanged,
                "Clipboard changed. Nothing was pasted.",
                cx,
            );
            return;
        }
        self.start_processing(
            input,
            super::processing::Purpose::Protected(snapshot),
            window,
            cx,
        );
    }
}
