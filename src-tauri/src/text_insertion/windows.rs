use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use arboard::{Clipboard, SetExtWindows};
use tauri::Manager;
use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};

const RESTORE_AFTER: Duration = Duration::from_secs(1);

static PASTE: Mutex<()> = Mutex::new(());

/// Waits ~1 s before restoring the clipboard; never call on the main thread.
pub fn insert_text(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    let _serialized = PASTE.lock().unwrap_or_else(|e| e.into_inner());

    release_hotkey_modifiers(app);
    thread::sleep(Duration::from_millis(100));

    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
    let original = clipboard.get_text().ok();
    clipboard
        .set()
        .exclude_from_monitoring()
        .text(text)
        .map_err(|e| format!("Failed to set clipboard: {}", e))?;
    let ours = unsafe { GetClipboardSequenceNumber() };
    thread::sleep(Duration::from_millis(100));

    if let Err(e) = send_keys(&[
        (VK_CONTROL, false),
        (VK_V, false),
        (VK_V, true),
        (VK_CONTROL, true),
    ]) {
        // the copy above is excluded from clipboard history
        let _ = clipboard.set_text(text);
        return Err(format!("{e}; transcript left on the clipboard"));
    }
    thread::sleep(RESTORE_AFTER);

    if let Some(original) = original {
        if unsafe { GetClipboardSequenceNumber() } == ours {
            let _ = clipboard.set().exclude_from_monitoring().text(original);
        }
    }
    Ok(())
}

/// The hotkey's modifiers can still read as held right after release.
fn release_hotkey_modifiers(app: &tauri::AppHandle) {
    let keycodes = match app.try_state::<crate::AppState>() {
        Some(state) => match state.current_hotkey.lock() {
            Ok(hotkey) => hotkey.modifier_keycodes(),
            Err(_) => return,
        },
        None => return,
    };
    let keys: Vec<_> = keycodes
        .iter()
        .map(|&vk| (VIRTUAL_KEY(vk as u16), true))
        .collect();
    let _ = send_keys(&keys);
}

fn send_keys(keys: &[(VIRTUAL_KEY, bool)]) -> Result<(), String> {
    let inputs: Vec<INPUT> = keys
        .iter()
        .map(|&(vk, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!(
            "SendInput inserted {} of {} key events",
            sent,
            inputs.len()
        ))
    }
}
