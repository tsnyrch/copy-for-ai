#[path = "shield_actions.rs"]
mod actions;
#[path = "shield_clipboard.rs"]
mod clipboard;
#[path = "shield_menu.rs"]
mod menu;
#[path = "shield_processing.rs"]
mod processing;
#[path = "shield_render.rs"]
mod render;
#[cfg(test)]
#[path = "shield_tests.rs"]
mod tests;

use crate::application::{
    NerService, OperationHistory, SettingsService, Shortcuts, StatisticsStore, Workflow,
};
use copy_for_ai_core::{Direction, EntityKind};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::*;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MainTab {
    Text,
    History,
    Statistics,
    Settings,
}

pub struct ShieldView {
    input: Entity<TextareaState>,
    output: Entity<TextareaState>,
    custom: Entity<InputState>,
    workflow: Workflow,
    native: std::sync::Arc<dyn crate::platform::PastePlatform>,
    history: OperationHistory,
    tab: MainTab,
    statistics_store: StatisticsStore,
    statistics_saving: bool,
    statistics_saved: u64,
    statistics_failed: bool,
    quitting: bool,
    settings: SettingsService,
    settings_page: settings::SettingsPage,
    preferences_busy: bool,
    login_busy: bool,
    login_status: crate::platform::login::LoginStatus,
    _appearance: Subscription,
    ner: NerService,
    rule_kind: EntityKind,
    status: String,
    error: bool,
    shortcuts: Option<Shortcuts>,
    busy: bool,
    paste_started: Option<Instant>,
    generation: u64,
    confirm_forget: bool,
    last_count: usize,
    status_bar: Option<crate::platform::StatusBar>,
    output_direction: Option<Direction>,
    _poll: Task<()>,
}

impl ShieldView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        match SettingsService::load() {
            Ok(settings) => Self::with_settings(settings, window, cx),
            Err(error) => {
                let mut view = Self::with_settings(SettingsService::temporary(), window, cx);
                view.status = format!("{error} Using temporary settings.");
                view.error = true;
                view
            }
        }
    }

    pub fn new_preview(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_settings(SettingsService::temporary(), window, cx)
    }

    fn with_settings(
        settings: SettingsService,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (statistics_store, lifetime, statistics_error) = if settings.is_persistent() {
            match StatisticsStore::load() {
                Ok((store, totals)) => (store, totals, None),
                Err(error) => (
                    StatisticsStore::resettable().unwrap_or_else(|_| StatisticsStore::temporary()),
                    Default::default(),
                    Some(error),
                ),
            }
        } else {
            (StatisticsStore::temporary(), Default::default(), None)
        };
        Self::apply_appearance(settings.preferences(), window, cx);
        let settings_view = cx.entity().downgrade();
        App::on_action(cx, move |_: &crate::OpenSettings, cx| {
            if let Some(view) = settings_view.upgrade() {
                view.update(cx, |this, cx| this.open_settings(cx));
            }
        });
        let quitting_view = cx.entity().downgrade();
        App::on_action(cx, move |_: &crate::Quit, cx| {
            if let Some(view) = quitting_view.upgrade() {
                view.update(cx, |this, cx| this.quit(cx));
            }
        });
        window.on_window_should_close(cx, |_, cx| {
            cx.defer(|cx| cx.dispatch_action(&crate::Quit));
            false
        });
        let weak = cx.entity().downgrade();
        let appearance = window.observe_window_appearance(move |window, cx| {
            if let Some(view) = weak.upgrade() {
                view.update(cx, |this, cx| {
                    if this.settings.preferences().appearance()
                        == crate::application::Appearance::System
                    {
                        Self::apply_appearance(this.settings.preferences(), window, cx);
                    }
                });
            }
        });
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(15)
                .placeholder("Paste text or load the clipboard…")
        });
        let output = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(15)
                .placeholder("The result will appear here.")
        });
        let custom = cx.new(|cx| {
            InputState::new(window, cx).placeholder("For example, Alice Morgan or a customer name")
        });
        let executor = cx.background_executor().clone();
        let poll = cx.spawn_in(window, async move |view, cx| {
            loop {
                executor.timer(Duration::from_millis(40)).await;
                if view
                    .update_in(cx, |this, window, cx| {
                        this.poll_processing_progress(cx);
                        this.poll_status_bar(window, cx);
                        this.flush_statistics(window, cx);
                    })
                    .is_err()
                {
                    return;
                }
                while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    if event.state != HotKeyState::Pressed {
                        continue;
                    }
                    if view
                        .update_in(cx, |this, window, cx| {
                            let Some(shortcuts) = this.shortcuts.as_ref() else {
                                return;
                            };
                            if shortcuts.is_paste(event.id) {
                                this.protected_paste(window, cx);
                            } else if shortcuts.is_restore(event.id) {
                                this.restore_clipboard(window, cx);
                            }
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                if view.upgrade().is_none() {
                    return;
                }
            }
        });
        Self {
            input,
            output,
            custom,
            workflow: Workflow::new(),
            native: std::sync::Arc::new(crate::platform::SystemPastePlatform),
            history: OperationHistory::with_lifetime(lifetime),
            tab: if settings.preferences().show_history() {
                MainTab::History
            } else {
                MainTab::Text
            },
            statistics_store,
            statistics_saving: false,
            statistics_saved: 0,
            statistics_failed: false,
            quitting: false,
            settings,
            settings_page: settings::SettingsPage::General,
            preferences_busy: false,
            login_busy: false,
            login_status: crate::platform::login::status(),
            _appearance: appearance,
            ner: NerService::new(),
            rule_kind: EntityKind::Person,
            status: statistics_error.unwrap_or("Ready.").into(),
            error: statistics_error.is_some(),
            shortcuts: None,
            busy: false,
            paste_started: None,
            generation: 0,
            confirm_forget: false,
            last_count: 0,
            status_bar: None,
            output_direction: None,
            _poll: poll,
        }
    }

    fn notice(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.status = text.into();
        self.error = false;
        cx.notify();
    }

    fn fail(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.reject(text, cx);
    }

    fn fail_processing(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.history
            .set_failure(crate::application::FailureReason::ProcessingFailure);
        self.history
            .finish(crate::application::OperationStatus::Failed, 0);
        self.fail(text, cx);
    }

    fn fail_processing_reason(
        &mut self,
        reason: crate::application::FailureReason,
        text: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.history.set_failure(reason);
        self.history
            .finish(crate::application::OperationStatus::Failed, 0);
        self.fail(text, cx);
    }

    fn reject(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.status = text.into();
        self.error = true;
        cx.notify();
    }
}

impl Drop for ShieldView {
    fn drop(&mut self) {
        self.ner.cancel();
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/long_document.rs"]
mod long_tests;

#[cfg(test)]
#[path = "../../../tests/ui/protected_paste.rs"]
mod paste_tests;

#[cfg(test)]
#[path = "../../../tests/ui/navigation.rs"]
mod navigation_tests;

#[path = "shield_preview.rs"]
mod preview;

#[path = "shield_settings.rs"]
mod settings;

#[path = "shield_history.rs"]
mod history_screen;
#[path = "shield_statistics.rs"]
mod statistics;

#[path = "shield_preferences.rs"]
mod preferences;

#[path = "shield_progress.rs"]
mod progress;
