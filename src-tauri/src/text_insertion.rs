use arboard::Clipboard;
use std::thread;
use std::time::Duration;

/// Clear any stuck modifier keys before typing.
/// Posts FlagsChanged events with null flags to reset the keyboard modifier state.
#[cfg(target_os = "macos")]
fn clear_modifiers(keycodes: &[i64]) {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, EventField};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Post FlagsChanged event with null flags for each modifier keycode
    for &keycode in keycodes {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            if let Ok(event) = CGEvent::new(source) {
                event.set_type(CGEventType::FlagsChanged);
                event.set_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE, keycode);
                event.set_flags(CGEventFlags::CGEventFlagNull);
                event.post(CGEventTapLocation::HID);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn clear_modifiers(_keycodes: &[i64]) {}

/// Insert text by copying to clipboard and pasting with Cmd+V.
/// Uses AppleScript on macOS for reliable keystroke simulation.
/// The keycodes parameter specifies which modifier keys to clear before pasting.
pub fn insert_text_via_clipboard(text: &str, modifier_keycodes: &[i64]) -> Result<(), String> {
    // Clear any stuck modifier keys first
    clear_modifiers(modifier_keycodes);
    thread::sleep(Duration::from_millis(100));

    // Save current clipboard content
    let mut clipboard = Clipboard::new()
        .map_err(|e| format!("Failed to access clipboard: {}", e))?;
    let original_content = clipboard.get_text().ok();

    // Set new content
    clipboard.set_text(text)
        .map_err(|e| format!("Failed to set clipboard: {}", e))?;

    thread::sleep(Duration::from_millis(100));

    // Simulate Cmd+V paste using AppleScript
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"v\" using command down")
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("osascript failed: {}", stderr));
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to create Enigo: {}", e))?;
        enigo.key(Key::Control, Direction::Press)
            .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
        enigo.key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| format!("Failed to click V: {}", e))?;
        enigo.key(Key::Control, Direction::Release)
            .map_err(|e| format!("Failed to release Ctrl: {}", e))?;
    }

    thread::sleep(Duration::from_millis(100));

    // Restore original clipboard content
    if let Some(original) = original_content {
        let _ = clipboard.set_text(&original);
    }

    Ok(())
}

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
