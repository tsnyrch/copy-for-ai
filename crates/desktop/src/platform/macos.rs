use super::Snapshot;
use objc2_app_kit::{NSPasteboard, NSWorkspace};
use objc2_foundation::{NSString, NSURL};

pub fn open_accessibility_settings() -> bool {
    NSURL::URLWithString(&NSString::from_str(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    ))
    .is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url))
}
use std::{ffi::c_void, ptr};

const COMMAND: u64 = 1 << 20;
const MODIFIERS: u64 = (1 << 17) | (1 << 18) | (1 << 19) | COMMAND;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPostToPid(pid: i32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
}

pub fn available() -> bool {
    true
}

pub fn accessibility_trusted() -> bool {
    // SAFETY: the function takes no pointers and only queries process trust.
    unsafe { AXIsProcessTrusted() }
}

pub fn clipboard_version() -> i64 {
    NSPasteboard::generalPasteboard().changeCount() as i64
}

pub fn target() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier())
}

pub fn modifiers_released() -> bool {
    // SAFETY: zero is the documented combined-session CGEventSourceStateID.
    unsafe { CGEventSourceFlagsState(0) & MODIFIERS == 0 }
}

/// Sends only Cmd+V to the captured PID. Never submits a prompt.
pub fn paste(snapshot: Snapshot) -> Result<(), &'static str> {
    if !accessibility_trusted() {
        return Err("Accessibility permission is missing. Paste stopped.");
    }
    if target() != Some(snapshot.target)
        || clipboard_version() != snapshot.clipboard_version
        || !modifiers_released()
    {
        return Err("Target changed or modifiers are pressed. Paste stopped.");
    }
    // SAFETY: null event source is supported; ANSI key code 9 is V. Returned CF
    // objects are checked for null and released exactly once after posting.
    unsafe {
        let down = CGEventCreateKeyboardEvent(ptr::null(), 9, true);
        let up = CGEventCreateKeyboardEvent(ptr::null(), 9, false);
        if down.is_null() || up.is_null() {
            if !down.is_null() {
                CFRelease(down);
            }
            if !up.is_null() {
                CFRelease(up);
            }
            return Err("Could not create keyboard event. Paste stopped.");
        }
        CGEventSetFlags(down, COMMAND);
        CGEventSetFlags(up, COMMAND);
        CGEventPostToPid(snapshot.target, down);
        CGEventPostToPid(snapshot.target, up);
        CFRelease(down);
        CFRelease(up);
    }
    Ok(())
}
