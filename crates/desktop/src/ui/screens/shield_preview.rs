use super::ShieldView;
use crate::application::OperationStatus;
use copy_for_ai_core::{Detector, Direction, EntityKind};
use gpui_kit::*;
use zeroize::Zeroizing;

impl ShieldView {
    pub fn show_settings_preview(&mut self, cx: &mut Context<Self>) {
        self.open_settings(cx);
        self.notice("Settings preview · preferences are not saved.", cx);
    }

    pub fn show_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = "Alice Morgan writes to alice@example.test.\nPlease prepare a short reply to this message.";
        self.history.begin(Some(Direction::Anonymize), input.len());
        let result = self
            .workflow
            .add_rule(EntityKind::Person, "Alice Morgan".into())
            .and_then(|()| {
                self.workflow.snapshot().finish_cancellable(
                    &Detector::new(),
                    Zeroizing::new(input.into()),
                    &[],
                    &|| false,
                )
            })
            .and_then(|pending| self.workflow.commit(pending));
        if let Ok(output) = result {
            self.input
                .update(cx, |state, cx| state.set_value(input, window, cx));
            self.output
                .update(cx, |state, cx| state.set_value(output.text, window, cx));
            self.output_direction = Some(Direction::Anonymize);
            self.history
                .finish(OperationStatus::Completed, output.replacements);
        }
        self.history.begin(Some(Direction::Restore), 32);
        self.history.finish(OperationStatus::Cancelled, 0);
        self.history.begin(Some(Direction::Unresolved), 24);
        self.history.finish(OperationStatus::Failed, 0);
        self.tab = super::MainTab::Text;
        self.notice("UI preview · synthetic data and sample operations.", cx);
    }
}
