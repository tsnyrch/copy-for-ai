use super::ShieldView;
use crate::{
    application::{Appearance, LICENSE_TEXT},
    platform::login::LoginStatus,
    ui::components::settings_row,
};
use gpui_kit::component::Disableable;
use gpui_kit::{
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
        switch::Switch,
    },
    prelude::*,
    *,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsPage {
    General,
    Appearance,
    Rules,
    Data,
    Applications,
    Updates,
    About,
}

impl SettingsPage {
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Rules => "Terms",
            Self::Data => "Sessions and data",
            Self::Applications => "Applications",
            Self::Updates => "Updates",
            Self::About => "About",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::General => "App startup and default behavior.",
            Self::Appearance => "Use the system appearance or choose your own.",
            Self::Rules => "Custom sensitive terms supplement local detection.",
            Self::Data => "Manage sessions and clear local totals.",
            Self::Applications => "Planned protection for selected apps and terminal tools.",
            Self::Updates => "App version and updates.",
            Self::About => "Local text protection when pasting.",
        }
    }
}

impl ShieldView {
    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let preferences = self.settings.preferences();
        let page = self.settings_page;
        let navigation = [
            SettingsPage::General,
            SettingsPage::Appearance,
            SettingsPage::Rules,
            SettingsPage::Data,
            SettingsPage::Applications,
            SettingsPage::Updates,
            SettingsPage::About,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, target)| {
            Button::new(("settings-page", index))
                .label(target.label())
                .when(target == page, |button| button.primary())
                .when(target != page, |button| button.ghost())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings_page = target;
                    cx.notify();
                }))
        })
        .collect::<Vec<_>>();
        let mut content = div()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .p_4()
            .flex()
            .flex_col()
            .gap_2();
        match page {
            SettingsPage::General => {
                let login_description = match self.login_status {
                    LoginStatus::RequiresApproval => "macOS requires approval in Login Items.",
                    LoginStatus::Unavailable => {
                        "Requires a supported macOS version and an installed .app bundle."
                    }
                    _ => "Open Copy for AI automatically when you log in to macOS.",
                };
                content = content
                    .child(settings_row("Launch at login", login_description,
                        Switch::new("launch-at-login").accessibility_label("Launch at login").checked(matches!(self.login_status, LoginStatus::Enabled | LoginStatus::RequiresApproval))
                            .disabled(!self.settings.is_persistent() || self.login_busy || self.login_status == LoginStatus::Unavailable)
                            .on_click(cx.listener(|this, enabled: &bool, window, cx| this.set_login_enabled(*enabled, window, cx))), cx))
                    .child(div().h(px(1.)).bg(theme.border))
                    .child(settings_row("History at startup", "Open the operation history at next startup. History remains in memory only.",
                        Switch::new("history-at-startup").accessibility_label("History at startup").checked(preferences.show_history()).disabled(self.preferences_busy)
                            .on_click(cx.listener(|this, checked: &bool, window, cx| this.save_preferences(this.settings.preferences().with_history(*checked), window, cx))), cx))
                    .child(div().h(px(1.)).bg(theme.border))
                    .child(settings_row("Global shortcuts", "Cmd+Shift+V processes and pastes. Cmd+Shift+R restores the clipboard only. Enabled for this app run.",
                        Switch::new("settings-shortcuts").accessibility_label("Global shortcuts").checked(self.shortcuts.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_shortcuts(cx))), cx));
            }
            SettingsPage::Appearance => {
                let choices = [
                    (Appearance::System, "System"),
                    (Appearance::Light, "Light"),
                    (Appearance::Dark, "Dark"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (appearance, label))| {
                    Button::new(("appearance-choice", index))
                        .label(label)
                        .disabled(self.preferences_busy)
                        .when(preferences.appearance() == appearance, |button| {
                            button.primary()
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.save_preferences(
                                this.settings.preferences().with_appearance(appearance),
                                window,
                                cx,
                            )
                        }))
                })
                .collect::<Vec<_>>();
                content = content.child(settings_row(
                    "Theme",
                    "System mode follows macOS appearance changes.",
                    div().flex().gap_2().children(choices),
                    cx,
                ));
            }
            SettingsPage::Rules => {
                let rows = self
                    .workflow
                    .rules()
                    .iter()
                    .enumerate()
                    .map(|(index, rule)| {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(rule.kind().tag())
                                    .child(rule.value().to_owned()),
                            )
                            .child(
                                Button::new(("remove-rule", index))
                                    .ghost()
                                    .label("Remove")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.workflow.remove_rule(index);
                                        this.notice("Rule removed.", cx);
                                    })),
                            )
                    })
                    .collect::<Vec<_>>();
                content = content.child(crate::ui::components::rule_editor(
                    &self.custom,
                    self.rule_kind,
                    self.busy,
                    rows,
                    cx.listener(|this, _, _, cx| this.cycle_rule_kind(cx)),
                    cx.listener(|this, _, window, cx| this.add_rule(window, cx)),
                    cx,
                ));
            }
            SettingsPage::Data => {
                content = content
                    .child(settings_row("New session", "Older local markers can still be restored. Input and output are kept.", Button::new("new-session").label("New session").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| {
                        match this.workflow.new_session() { Ok(()) => this.notice("New session created.", cx), Err(error) => this.fail(error.to_string(), cx) }
                    })), cx))
                    .child(settings_row("Forget everything", "Clears editors, sessions, custom terms, history and lifetime statistics. Keeps ordinary preferences.", Button::new("forget").danger().label(if self.confirm_forget { "Confirm forgetting" } else { "Forget everything" }).on_click(cx.listener(|this, _, window, cx| this.forget(window, cx))), cx))
                    .child(settings_row("Clipboard processing", "The same workflow without system paste; changes only the clipboard.", Button::new("process-clipboard").label("Process clipboard").disabled(self.busy).on_click(cx.listener(|this, _, window, cx| this.process_clipboard(window, cx))), cx))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Session {} · {} sessions · {} entries", self.workflow.scope(), self.workflow.session_count(), self.workflow.entity_count())));
            }
            SettingsPage::Applications => {
                content = content
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("TODO · automatic detection of selected apps"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Planned in roadmap M5. No apps or terminal processes are monitored yet, and ordinary Cmd+V is not intercepted."));
                for (index, label) in ["Zed", "Delta", "Codex", "Claude"].into_iter().enumerate() {
                    content = content.child(settings_row(
                        label,
                        "App / editor and terminal tool · planned",
                        Switch::new(("future-app", index))
                            .accessibility_label(label)
                            .checked(false)
                            .disabled(true),
                        cx,
                    ));
                }
            }
            SettingsPage::Updates => {
                content = content
                    .child(settings_row("Current version", "Updates are currently manual. No signed release source is configured.", div().text_sm().child(env!("CARGO_PKG_VERSION")), cx))
                    .child(settings_row("Update check", "TODO · verified release source, package signature and safe updates preserving the current session.", Button::new("check-updates").label("Check for updates").disabled(true), cx));
            }
            SettingsPage::About => {
                content = content
                    .child(div().text_lg().font_weight(FontWeight::BOLD).child(format!("Copy for AI {}", env!("CARGO_PKG_VERSION"))))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Standalone native app. User text, mappings and history stay in memory; only ordinary preferences and aggregate statistics without document content are saved to disk."))
                    .child(div().text_sm().child("App license: MIT. Models and dependencies have their own licenses."))
                    .child(div().id("mit-license").text_xs().text_color(theme.muted_foreground).child(LICENSE_TEXT));
            }
        }
        div()
            .flex()
            .flex_col()
            .gap_4()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .p_1()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .children(navigation),
            )
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(page.label()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(page.description()),
                    )
                    .child(content),
            )
    }
}
