#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    Disabled,
    Enabled,
    RequiresApproval,
    Unavailable,
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos {
    use super::LoginStatus;
    use objc2::{
        class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, Bool},
    };
    use objc2_foundation::NSError;

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    fn service() -> Option<Retained<AnyObject>> {
        let class = AnyClass::get(c"SMAppService")?;
        // SAFETY: macOS 13+ SMAppService owns the returned main-app service object;
        // Retained keeps it alive for subsequent Objective-C calls.
        unsafe { msg_send![class, mainAppService] }
    }

    pub fn status() -> LoginStatus {
        let Some(service) = service() else {
            return LoginStatus::Unavailable;
        };
        // SAFETY: service is a retained SMAppService and status returns NSUInteger.
        let status: usize = unsafe { msg_send![&*service, status] };
        match status {
            0 => LoginStatus::Disabled,
            1 => LoginStatus::Enabled,
            2 => LoginStatus::RequiresApproval,
            _ => LoginStatus::Unavailable,
        }
    }

    pub fn set_enabled(enabled: bool) -> Result<LoginStatus, &'static str> {
        // SAFETY: NSBundle is a Foundation class; mainBundle and bundlePath return
        // retained Foundation objects. The path only validates packaging, not input.
        let bundle: Retained<AnyObject> = unsafe { msg_send![class!(NSBundle), mainBundle] };
        // SAFETY: bundle is a retained NSBundle; bundlePath returns a Foundation
        // NSString whose lifetime is held by the Retained binding.
        let path: Retained<objc2_foundation::NSString> = unsafe { msg_send![&*bundle, bundlePath] };
        if !path.to_string().ends_with(".app") {
            return Err("Launch at login requires an installed .app bundle.");
        }
        let service =
            service().ok_or("This macOS version does not support launch-at-login registration.")?;
        let mut error: Option<Retained<NSError>> = None;
        // SAFETY: service is SMAppService; both selectors take a writable NSError
        // out-parameter, valid until the synchronous call returns. No content is logged.
        let success: Bool = unsafe {
            if enabled {
                msg_send![&*service, registerAndReturnError: &mut error]
            } else {
                msg_send![&*service, unregisterAndReturnError: &mut error]
            }
        };
        if !success.as_bool() {
            return Err(
                "macOS rejected the launch-at-login change. The app must be installed correctly.",
            );
        }
        Ok(status())
    }
}

#[cfg(target_os = "macos")]
pub use macos::{set_enabled, status};
#[cfg(not(target_os = "macos"))]
pub fn status() -> LoginStatus {
    LoginStatus::Unavailable
}
#[cfg(not(target_os = "macos"))]
pub fn set_enabled(_: bool) -> Result<LoginStatus, &'static str> {
    Err("Launch at login is currently available only on macOS.")
}
