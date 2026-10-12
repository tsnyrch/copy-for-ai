#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusBarState {
    Paused,
    PermissionRequired,
    Ready,
    Processing,
}

impl StatusBarState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Paused => "Shortcuts disabled",
            Self::PermissionRequired => "Shortcuts enabled · Accessibility missing",
            Self::Ready => "Protected paste ready",
            Self::Processing => "Processing text…",
        }
    }
}

pub enum StatusBarAction {
    OpenDashboard,
    OpenSettings,
    ToggleShortcuts,
    RestoreClipboard,
    OpenPermissions,
    Quit,
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
#[path = "status_bar_macos.rs"]
mod native;
#[cfg(target_os = "macos")]
pub use native::StatusBar;

#[cfg(not(target_os = "macos"))]
pub struct StatusBar;

#[cfg(not(target_os = "macos"))]
impl StatusBar {
    pub fn new() -> Result<Self, &'static str> {
        Err("Menu bar is available only on macOS.")
    }

    pub fn next_action(&self) -> Option<StatusBarAction> {
        None
    }

    pub fn update(&mut self, _: StatusBarState) {}
}
