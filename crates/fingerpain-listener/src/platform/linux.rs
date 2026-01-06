//! Linux-specific active app detection using X11

use super::{ActiveApp, PlatformError};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, GetPropertyReply};
use x11rb::rust_connection::RustConnection;

pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    // Connect to X11 server
    let (conn, screen_num) = RustConnection::connect(None)
        .map_err(|e| PlatformError::GetActiveApp(format!("X11 connection failed: {}", e)))?;

    let setup = conn.setup();
    let screen = &setup.roots[screen_num];
    let root = screen.root;

    // Get the _NET_ACTIVE_WINDOW atom
    let active_window_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to intern atom: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get atom reply: {}", e)))?
        .atom;

    // Get the active window
    let active_window_reply = conn
        .get_property(false, root, active_window_atom, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property reply: {}", e)))?;

    if active_window_reply.value.len() < 4 {
        return Err(PlatformError::GetActiveApp("No active window".to_string()));
    }

    let active_window = u32::from_ne_bytes([
        active_window_reply.value[0],
        active_window_reply.value[1],
        active_window_reply.value[2],
        active_window_reply.value[3],
    ]);

    if active_window == 0 {
        return Err(PlatformError::GetActiveApp("No active window".to_string()));
    }

    // Get _NET_WM_NAME (UTF-8 window title)
    let wm_name_atom = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to intern atom: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get atom reply: {}", e)))?
        .atom;

    let utf8_string_atom = conn
        .intern_atom(false, b"UTF8_STRING")
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to intern atom: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get atom reply: {}", e)))?
        .atom;

    let name_reply = conn
        .get_property(false, active_window, wm_name_atom, utf8_string_atom, 0, 1024)
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property reply: {}", e)))?;

    let name = String::from_utf8_lossy(&name_reply.value).into_owned();

    // Get WM_CLASS for the bundle_id equivalent
    let wm_class_atom = conn
        .intern_atom(false, b"WM_CLASS")
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to intern atom: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get atom reply: {}", e)))?
        .atom;

    let class_reply = conn
        .get_property(false, active_window, wm_class_atom, AtomEnum::STRING, 0, 1024)
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property: {}", e)))?
        .reply()
        .map_err(|e| PlatformError::GetActiveApp(format!("Failed to get property reply: {}", e)))?;

    // WM_CLASS contains two null-terminated strings: instance name and class name
    let class_str = String::from_utf8_lossy(&class_reply.value);
    let bundle_id = class_str
        .split('\0')
        .nth(1)
        .unwrap_or(&class_str)
        .to_string();

    Ok(ActiveApp {
        name: if name.is_empty() { "Unknown".to_string() } else { name },
        bundle_id: if bundle_id.is_empty() { "unknown".to_string() } else { bundle_id },
    })
}
