#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::insert_text;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::insert_text;

/// Check if accessibility permission is granted
pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Use the official macOS API for checking accessibility trust
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }

        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Preferences to Accessibility settings
pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| format!("Failed to open System Preferences: {}", e))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this platform".to_string())
    }
}

/// Check if input monitoring permission is granted using IOHIDRequestAccess.
/// This API both checks and requests Input Monitoring permission.
/// Returns true if permission is granted.
pub fn check_input_monitoring_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "IOKit", kind = "framework")]
        extern "C" {
            fn IOHIDRequestAccess(request: u32) -> bool;
        }

        const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
        unsafe { IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open Input Monitoring privacy settings
pub fn open_input_monitoring_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .spawn()
            .map_err(|e| format!("Failed to open System Preferences: {}", e))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this platform".to_string())
    }
}
