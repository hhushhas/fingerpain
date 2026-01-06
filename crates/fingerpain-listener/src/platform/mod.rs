//! Platform-specific functionality for active app detection

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Failed to get active app: {0}")]
    GetActiveApp(String),
    #[error("Unsupported platform")]
    Unsupported,
}

/// Information about the currently active application
#[derive(Debug, Clone)]
pub struct ActiveApp {
    /// Display name of the application
    pub name: String,
    /// Bundle ID (macOS), process name (Windows/Linux)
    pub bundle_id: String,
}

/// Get the currently active application
#[cfg(target_os = "macos")]
pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    macos::get_active_app()
}

#[cfg(target_os = "windows")]
pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    windows::get_active_app()
}

#[cfg(target_os = "linux")]
pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    linux::get_active_app()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    Err(PlatformError::Unsupported)
}
