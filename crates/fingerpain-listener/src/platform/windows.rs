//! Windows-specific active app detection

use super::{ActiveApp, PlatformError};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

pub fn get_active_app() -> Result<ActiveApp, PlatformError> {
    unsafe {
        // Get the foreground window
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return Err(PlatformError::GetActiveApp(
                "No foreground window".to_string(),
            ));
        }

        // Get the window title
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = if len > 0 {
            OsString::from_wide(&title_buf[..len as usize])
                .to_string_lossy()
                .into_owned()
        } else {
            "Unknown".to_string()
        };

        // Get the process ID
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        // Get process name from process ID
        let process_name = get_process_name(process_id).unwrap_or_else(|| "unknown".to_string());

        Ok(ActiveApp {
            name: title,
            bundle_id: process_name,
        })
    }
}

fn get_process_name(process_id: u32) -> Option<String> {
    use windows::Win32::System::ProcessStatus::K32GetModuleBaseNameW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id).ok()?;

        let mut name_buf = [0u16; 260];
        let len = K32GetModuleBaseNameW(handle, None, &mut name_buf);

        if len > 0 {
            Some(
                OsString::from_wide(&name_buf[..len as usize])
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            None
        }
    }
}
