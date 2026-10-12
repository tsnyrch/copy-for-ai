#[derive(Clone, Copy)]
pub struct Snapshot {
    pub clipboard_version: i64,
    pub target: i32,
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
pub use unsupported::*;
mod status_bar;
pub use status_bar::{StatusBar, StatusBarAction, StatusBarState};

/// OS boundary shared by protected paste and its isolated test adapter.
pub trait PastePlatform: Send + Sync {
    fn accessibility_trusted(&self) -> bool;
    fn clipboard_version(&self) -> i64;
    fn target(&self) -> Option<i32>;
    fn modifiers_released(&self) -> bool;
    fn paste(&self, snapshot: Snapshot) -> Result<(), &'static str>;

    fn snapshot(&self) -> Option<Snapshot> {
        Some(Snapshot {
            clipboard_version: self.clipboard_version(),
            target: self.target()?,
        })
    }
}

pub struct SystemPastePlatform;

impl PastePlatform for SystemPastePlatform {
    fn accessibility_trusted(&self) -> bool {
        accessibility_trusted()
    }
    fn clipboard_version(&self) -> i64 {
        clipboard_version()
    }
    fn target(&self) -> Option<i32> {
        target()
    }
    fn modifiers_released(&self) -> bool {
        modifiers_released()
    }
    fn paste(&self, snapshot: Snapshot) -> Result<(), &'static str> {
        paste(snapshot)
    }
}

pub mod login;
