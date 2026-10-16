use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tauri::Manager;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    KBDLLHOOKSTRUCT_FLAGS, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use super::recording::{set_hotkey_held, HOTKEY_ACTIVE};
use crate::AppState;

// Track currently held keys (VK codes)
lazy_static::lazy_static! {
    static ref HELD_KEYS: Mutex<HashSet<i64>> = Mutex::new(HashSet::new());
    static ref APP_HANDLE: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);
}

/// Clear the held keys set (called when hotkey config changes)
pub fn clear_held_keys() {
    if let Ok(mut held) = HELD_KEYS.lock() {
        held.clear();
    }
    HOTKEY_ACTIVE.store(false, Ordering::SeqCst);
}

/// Set up a low-level keyboard hook and run its message pump on a dedicated thread.
/// Returns immediately — the hook runs in the background.
pub fn setup_event_tap(app_handle: tauri::AppHandle) -> Result<(), String> {
    println!("[HOTKEY] Setting up Windows keyboard hook...");

    // Store app handle for the hook callback
    {
        let mut handle = APP_HANDLE.lock().map_err(|e| e.to_string())?;
        *handle = Some(app_handle);
    }

    // Low-level keyboard hook must be installed on a thread with a message pump
    std::thread::spawn(|| {
        unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0);
            match hook {
                Ok(h) => {
                    println!("[HOTKEY] Keyboard hook installed");
                    // Run the message pump — this blocks
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        // Dispatch not needed for low-level hooks, but pump must run
                    }
                    let _ = UnhookWindowsHookEx(h);
                }
                Err(e) => {
                    eprintln!("[HOTKEY] Failed to install keyboard hook: {:?}", e);
                }
            }
        }
    });

    println!("[HOTKEY] Windows keyboard hook thread started");
    Ok(())
}

/// Low-level keyboard hook callback
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode as i64;
        let msg = w_param.0 as u32;

        // Determine the actual side-specific VK code
        let vk_code = disambiguate_vk(vk_code, kb_struct);

        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        if is_down || is_up {
            update_held_key(vk_code, is_down);
            check_hotkey_state();
        }
    }

    CallNextHookEx(Some(HHOOK::default()), n_code, w_param, l_param)
}

/// Disambiguate generic VK_SHIFT/VK_CONTROL/VK_MENU into left/right variants
/// using the scan code and extended-key flag from the hook struct.
fn disambiguate_vk(vk_code: i64, kb: &KBDLLHOOKSTRUCT) -> i64 {
    let extended = kb.flags.contains(KBDLLHOOKSTRUCT_FLAGS(1)); // LLKHF_EXTENDED
    match vk_code {
        // VK_SHIFT → left or right based on scan code
        0x10 => {
            if kb.scanCode == 0x36 {
                0xA1
            } else {
                0xA0
            } // VK_RSHIFT : VK_LSHIFT
        }
        // VK_CONTROL → left or right based on extended flag
        0x11 => {
            if extended {
                0xA3
            } else {
                0xA2
            } // VK_RCONTROL : VK_LCONTROL
        }
        // VK_MENU → left or right based on extended flag
        0x12 => {
            if extended {
                0xA5
            } else {
                0xA4
            } // VK_RMENU : VK_LMENU
        }
        other => other,
    }
}

fn update_held_key(vk_code: i64, pressed: bool) {
    if let Ok(mut held) = HELD_KEYS.lock() {
        if pressed {
            held.insert(vk_code);
        } else {
            held.remove(&vk_code);
        }
    }
}

fn check_hotkey_state() {
    let app_handle = match APP_HANDLE.lock() {
        Ok(guard) => match guard.as_ref() {
            Some(h) => h.clone(),
            None => return,
        },
        Err(_) => return,
    };

    // Get required keycodes from config
    let required_keycodes = match app_handle.try_state::<AppState>() {
        Some(state) => match state.current_hotkey.lock() {
            Ok(hotkey) => hotkey.required_keycodes(),
            Err(_) => return,
        },
        None => return,
    };

    let all_held = match HELD_KEYS.lock() {
        Ok(held) => required_keycodes.iter().all(|kc| held.contains(kc)),
        Err(_) => false,
    };

    set_hotkey_held(&app_handle, all_held);
}
