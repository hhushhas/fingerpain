//! macOS-specific active app detection using Cocoa/AppKit

use super::{ActiveApp, PlatformError};
use cocoa::base::{id, nil};
use objc::{class, msg_send, sel, sel_impl};

pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    unsafe {
        // Get the shared workspace
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return Err(PlatformError::GetActiveApp(
                "Failed to get shared workspace".to_string(),
            ));
        }

        // Get the frontmost application
        let frontmost: id = msg_send![workspace, frontmostApplication];
        if frontmost == nil {
            return Err(PlatformError::GetActiveApp(
                "No frontmost application".to_string(),
            ));
        }

        // Get the localized name
        let name: id = msg_send![frontmost, localizedName];
        let name = if name != nil {
            nsstring_to_string(name)
        } else {
            "Unknown".to_string()
        };

        // Get the bundle identifier
        let bundle_id: id = msg_send![frontmost, bundleIdentifier];
        let bundle_id = if bundle_id != nil {
            nsstring_to_string(bundle_id)
        } else {
            "unknown".to_string()
        };

        Ok(ActiveApp { name, bundle_id })
    }
}

unsafe fn nsstring_to_string(nsstring: id) -> String {
    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
    if bytes.is_null() {
        return String::new();
    }
    std::ffi::CStr::from_ptr(bytes)
        .to_string_lossy()
        .into_owned()
}
