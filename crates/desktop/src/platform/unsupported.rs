use super::Snapshot;
pub fn available() -> bool {
    false
}
pub fn accessibility_trusted() -> bool {
    false
}
pub fn clipboard_version() -> i64 {
    0
}
pub fn target() -> Option<i32> {
    None
}
pub fn modifiers_released() -> bool {
    false
}
pub fn paste(_: Snapshot) -> Result<(), &'static str> {
    Err("Automatic paste is supported only on macOS in this version.")
}
pub fn open_accessibility_settings() -> bool {
    false
}
