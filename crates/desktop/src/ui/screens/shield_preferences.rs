use super::ShieldView;
use crate::{
    application::{Appearance, Preferences},
    platform::login::{self, LoginStatus},
};
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};

impl ShieldView {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.tab = super::MainTab::Settings;
        self.login_status = login::status();
        cx.notify();
    }

    pub(super) fn apply_appearance(preferences: Preferences, window: &mut Window, cx: &mut App) {
        match preferences.appearance() {
            Appearance::System => {
                cx.set_window_appearance(None);
                Theme::sync_system_appearance(Some(window), cx);
            }
            Appearance::Light => {
                cx.set_window_appearance(Some(WindowAppearance::Light));
                Theme::change(ThemeMode::Light, Some(window), cx);
            }
            Appearance::Dark => {
                cx.set_window_appearance(Some(WindowAppearance::Dark));
                Theme::change(ThemeMode::Dark, Some(window), cx);
            }
        }
        crate::ui::theme::configure(cx);
    }

    pub(super) fn save_preferences(
        &mut self,
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preferences_busy {
            return;
        }
        self.preferences_busy = true;
        let saving = self.settings.save(preferences);
        cx.notify();
        cx.spawn_in(window, async move |view, cx| {
            let result = saving.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.preferences_busy = false;
                match result {
                    Ok(preferences) => {
                        this.settings.accept(preferences);
                        Self::apply_appearance(preferences, window, cx);
                        this.notice(
                            if this.settings.is_persistent() {
                                "Settings saved."
                            } else {
                                "Settings applied for this run."
                            },
                            cx,
                        );
                    }
                    Err(error) => this.fail(error, cx),
                }
            });
        })
        .detach();
    }

    pub(super) fn set_login_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.login_busy {
            return;
        }
        self.login_busy = true;
        cx.notify();
        cx.spawn_in(window, async move |view, cx| {
            let result = smol::unblock(move || login::set_enabled(enabled)).await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.login_busy = false;
                this.login_status = login::status();
                match result {
                    Ok(LoginStatus::RequiresApproval) => this.notice(
                        "Registration is awaiting your approval in macOS · Login Items.",
                        cx,
                    ),
                    Ok(_) => this.notice("Launch-at-login setting changed.", cx),
                    Err(error) => this.fail(error, cx),
                }
            });
        })
        .detach();
    }
}
