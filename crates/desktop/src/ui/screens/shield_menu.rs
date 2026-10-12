use super::ShieldView;
use crate::platform::{self, StatusBar, StatusBarAction, StatusBarState};
use gpui_kit::*;

impl ShieldView {
    pub fn install_status_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match StatusBar::new() {
            Ok(bar) => {
                self.status_bar = Some(bar);
                window.on_window_should_close(cx, |_, cx| {
                    cx.hide();
                    false
                });
            }
            Err(error) => self.fail(error, cx),
        }
    }

    pub(super) fn poll_status_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(action) = self.status_bar.as_ref().and_then(StatusBar::next_action) {
            match action {
                StatusBarAction::OpenDashboard => {
                    self.tab = super::MainTab::Text;
                    cx.notify();
                    cx.activate(true);
                    window.activate_window();
                }
                StatusBarAction::OpenSettings => {
                    self.open_settings(cx);
                    cx.activate(true);
                    window.activate_window();
                }
                StatusBarAction::ToggleShortcuts => self.toggle_shortcuts(cx),
                StatusBarAction::RestoreClipboard => self.restore_clipboard(window, cx),
                StatusBarAction::OpenPermissions => {
                    if !platform::open_accessibility_settings() {
                        self.fail(
                            "Open System Settings → Privacy & Security → Accessibility.",
                            cx,
                        );
                    }
                }
                StatusBarAction::Quit => {
                    self.quit(cx);
                    return;
                }
            }
        }
        let state = if self.shortcuts.is_none() {
            StatusBarState::Paused
        } else if self.busy {
            StatusBarState::Processing
        } else if !self.native.accessibility_trusted() {
            StatusBarState::PermissionRequired
        } else {
            StatusBarState::Ready
        };
        if let Some(bar) = self.status_bar.as_mut() {
            bar.update(state);
        }
    }
}
