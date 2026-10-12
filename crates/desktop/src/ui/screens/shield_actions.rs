use super::ShieldView;
use copy_for_ai_core::EntityKind;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::*;
use zeroize::Zeroizing;

impl ShieldView {
    pub(super) fn load_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match Self::read_clipboard(cx) {
            Ok(text) => {
                self.input
                    .update(cx, |state, cx| state.set_value(text.as_str(), window, cx));
                self.output_direction = None;
                self.output
                    .update(cx, |state, cx| state.set_value("", window, cx));
                self.notice(
                    "Clipboard loaded. Processing automatically selects anonymization or restoration.",
                    cx,
                );
            }
            Err(error) => self.fail(error, cx),
        }
    }

    pub(super) fn process_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = Zeroizing::new(self.input.read(cx).value().to_string());
        self.start_processing(text, super::processing::Purpose::Preview, window, cx);
    }

    pub(super) fn process_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let version = self.native.clipboard_version();
        match Self::read_clipboard(cx) {
            Ok(text) => self.start_processing(
                text,
                super::processing::Purpose::Clipboard(version),
                window,
                cx,
            ),
            Err(error) => {
                self.history.begin(None, 0);
                self.fail_processing(error, cx);
            }
        }
    }

    pub(super) fn copy_output(&mut self, cx: &mut Context<Self>) {
        if self.output_direction.is_none()
            || self.output_direction == Some(copy_for_ai_core::Direction::Unresolved)
        {
            self.fail("First process input with a known local mapping.", cx);
            return;
        }
        let text = self.output.read(cx).value().to_string();
        if text.is_empty() {
            self.fail("Output is empty.", cx);
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notice("Output copied as plain text.", cx);
    }

    pub(super) fn add_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.custom.read(cx).value().to_string();
        match self.workflow.add_rule(self.rule_kind, value) {
            Ok(()) => {
                self.custom
                    .update(cx, |state, cx| state.set_value("", window, cx));
                self.notice("Custom term added. Applies to the next anonymization.", cx);
            }
            Err(error) => self.fail(error.to_string(), cx),
        }
    }

    pub(super) fn cycle_rule_kind(&mut self, cx: &mut Context<Self>) {
        self.rule_kind = match self.rule_kind {
            EntityKind::Person => EntityKind::Organization,
            EntityKind::Organization => EntityKind::Term,
            _ => EntityKind::Person,
        };
        cx.notify();
    }

    pub(super) fn forget(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_forget {
            self.confirm_forget = true;
            self.fail(
                "Confirm clearing sessions, editors, history and lifetime statistics. Old replies will no longer be restorable.",
                cx,
            );
            return;
        }
        self.generation += 1;
        self.busy = false;
        self.ner.forget();
        self.workflow.forget();
        self.history.clear();
        self.statistics_failed = false;
        self.input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(15)
                .placeholder("Paste text or load the clipboard…")
        });
        self.output = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(15)
                .placeholder("The result will appear here.")
        });
        self.custom =
            cx.new(|cx| InputState::new(window, cx).placeholder("Custom name or company"));
        self.last_count = 0;
        self.output_direction = None;
        self.confirm_forget = false;
        self.notice(
            "Sessions, custom terms, history and lifetime statistics forgotten. Clipboard unchanged.",
            cx,
        );
    }
}
