use core_foundation::base::TCFType;
use core_foundation::runloop::kCFRunLoopCommonModes;
use core_foundation_sys::runloop::{CFRunLoopAddSource, CFRunLoopGetMain};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{Emitter, Manager};

use crate::{audio, config, sidecar, text_insertion, AppState};

// Track whether the hotkey combo is currently activated
static HOTKEY_ACTIVE: AtomicBool = AtomicBool::new(false);

// Track currently held keys
lazy_static::lazy_static! {
    static ref HELD_KEYS: Mutex<HashSet<i64>> = Mutex::new(HashSet::new());
}

/// Clear the held keys set (called when hotkey config changes)
pub fn clear_held_keys() {
    if let Ok(mut held) = HELD_KEYS.lock() {
        held.clear();
    }
    HOTKEY_ACTIVE.store(false, Ordering::SeqCst);
}

// Store the mach port pointer for re-enabling the tap.
// Safety: The raw pointer is from CFMachPort which is thread-safe. We only read it
// after initialization (via OnceLock) and only use it to call CGEventTapEnable.
static TAP_MACH_PORT: OnceLock<usize> = OnceLock::new();

// FFI declaration for CGEventTapEnable (not exposed by core-graphics crate)
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: *mut std::ffi::c_void, enable: bool);
}

/// Re-enable the event tap if it was disabled by macOS
fn reenable_tap() {
    if let Some(&port_addr) = TAP_MACH_PORT.get() {
        println!("[HOTKEY] Re-enabling event tap...");
        unsafe {
            CGEventTapEnable(port_addr as *mut std::ffi::c_void, true);
        }
    }
}

/// Set up the event tap and attach it to the main run loop.
/// This function returns immediately - it does NOT block.
/// The event tap will receive events as part of the main run loop.
pub fn setup_event_tap(app_handle: tauri::AppHandle) -> Result<(), String> {
    println!("[HOTKEY] Creating CGEventTap...");
    let ax_trusted = crate::text_insertion::check_accessibility_permission();
    println!("[HOTKEY] AXIsProcessTrusted: {}", ax_trusted);

    // Note: TapDisabledByTimeout and TapDisabledByUserInput are NOT included in the mask
    // because they have special sentinel values (0xFFFFFFFE, 0xFFFFFFFF) that overflow
    // when the core-graphics crate tries to create a bitmask. These events are automatically
    // delivered to the callback when the tap is disabled, regardless of the mask.
    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        move |_proxy, event_type, event| {
            // Check if tap was disabled and re-enable it
            if matches!(
                event_type,
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
            ) {
                println!("[HOTKEY] Event tap was disabled by macOS, re-enabling...");
                reenable_tap();
                return CallbackResult::Keep;
            }

            handle_event(&app_handle, event_type, event);
            CallbackResult::Keep
        },
    )
    .map_err(|e| {
        let msg = format!(
            "Failed to create event tap. AXIsProcessTrusted={}, Error: {:?}",
            crate::text_insertion::check_accessibility_permission(),
            e
        );
        eprintln!("[HOTKEY] {}", msg);
        msg
    })?;

    // Store mach port pointer for re-enabling later
    let port_addr = tap.mach_port().as_concrete_TypeRef() as usize;
    let _ = TAP_MACH_PORT.set(port_addr);

    tap.enable();

    let run_loop_source = tap
        .mach_port()
        .create_runloop_source(0)
        .map_err(|_| "Failed to create run loop source")?;

    // KEY FIX: Add to MAIN run loop, not current thread's run loop
    // This ensures events are delivered even when the app is backgrounded,
    // because Tauri keeps the main run loop alive.
    unsafe {
        let main_run_loop = CFRunLoopGetMain();
        CFRunLoopAddSource(
            main_run_loop,
            run_loop_source.as_concrete_TypeRef(),
            kCFRunLoopCommonModes,
        );
    }

    // Keep tap and run loop source alive for app lifetime (intentional leak)
    // These must not be dropped or the event tap will stop working
    std::mem::forget(tap);
    std::mem::forget(run_loop_source);

    // DO NOT call CFRunLoop::run_current() - main run loop is already running via Tauri

    println!("[HOTKEY] Event tap attached to main run loop");
    Ok(())
}

fn handle_event(app_handle: &tauri::AppHandle, event_type: CGEventType, event: &CGEvent) {
    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
    let flags = event.get_flags();

    // Get current hotkey configuration from state
    let required_keycodes = match app_handle.try_state::<AppState>() {
        Some(state) => match state.current_hotkey.lock() {
            Ok(hotkey) => hotkey.required_keycodes(),
            Err(_) => return,
        },
        None => return,
    };

    match event_type {
        CGEventType::FlagsChanged => {
            // For modifier keys, we need to check if they're pressed or released
            // by checking the event flags
            let is_pressed = is_modifier_pressed(keycode, flags);

            update_held_key(keycode, is_pressed);
        }
        CGEventType::KeyDown => {
            // Non-modifier key pressed
            if !config::is_modifier_keycode(keycode) {
                update_held_key(keycode, true);
            }
        }
        CGEventType::KeyUp => {
            // Non-modifier key released
            if !config::is_modifier_keycode(keycode) {
                update_held_key(keycode, false);
            }
        }
        _ => {}
    }

    // Check if hotkey combo is now active
    let all_held = match HELD_KEYS.lock() {
        Ok(held) => required_keycodes.iter().all(|kc| held.contains(kc)),
        Err(_) => false,
    };

    let was_active = HOTKEY_ACTIVE.load(Ordering::SeqCst);

    if all_held && !was_active {
        HOTKEY_ACTIVE.store(true, Ordering::SeqCst);
        on_hotkey_pressed(app_handle);
    } else if !all_held && was_active {
        HOTKEY_ACTIVE.store(false, Ordering::SeqCst);
        on_hotkey_released(app_handle);
    }
}

/// Check if a modifier key is currently pressed based on event flags
fn is_modifier_pressed(keycode: i64, flags: CGEventFlags) -> bool {
    match keycode {
        // Command keys (54 = right, 55 = left)
        54 | 55 => flags.contains(CGEventFlags::CGEventFlagCommand),
        // Shift keys (56 = left, 60 = right)
        56 | 60 => flags.contains(CGEventFlags::CGEventFlagShift),
        // Option/Alt keys (58 = left, 61 = right)
        58 | 61 => flags.contains(CGEventFlags::CGEventFlagAlternate),
        // Control keys (59 = left, 62 = right)
        59 | 62 => flags.contains(CGEventFlags::CGEventFlagControl),
        // Caps Lock (57)
        57 => flags.contains(CGEventFlags::CGEventFlagAlphaShift),
        // Function key (63)
        63 => flags.contains(CGEventFlags::CGEventFlagSecondaryFn),
        _ => false,
    }
}

/// Update the held keys set
fn update_held_key(keycode: i64, pressed: bool) {
    if let Ok(mut held) = HELD_KEYS.lock() {
        if pressed {
            held.insert(keycode);
        } else {
            held.remove(&keycode);
        }
    }
}

fn on_hotkey_pressed(app_handle: &tauri::AppHandle) {
    println!("[DEBUG] Hotkey pressed - starting recording");
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // Check if sidecar is ready before allowing recording
        if let Some(state) = handle.try_state::<AppState>() {
            let sidecar_ready = state.sidecar_ready.lock().await;
            if !*sidecar_ready {
                println!("[DEBUG] Hotkey pressed but sidecar not ready");
                audio::play_busy_sound();
                return;
            }
            drop(sidecar_ready); // Release lock before acquiring is_recording lock

            let mut is_recording = state.is_recording.lock().await;
            *is_recording = true;
        }

        audio::play_start_sound();

        if let Err(e) = audio::start_recording() {
            eprintln!("[DEBUG] Failed to start recording: {}", e);
            return;
        }

        println!("[DEBUG] Recording started successfully");
        crate::tray::set_recording_state(&handle, true);
        let _ = handle.emit("recording-started", ());
    });
}

fn on_hotkey_released(app_handle: &tauri::AppHandle) {
    let handle = app_handle.clone();

    // Get modifier keycodes for clearing
    let modifier_keycodes: Vec<i64> = match app_handle.try_state::<AppState>() {
        Some(state) => match state.current_hotkey.lock() {
            Ok(hotkey) => hotkey.modifier_keycodes(),
            Err(_) => vec![54], // fallback to right command
        },
        None => vec![54],
    };

    tauri::async_runtime::spawn(async move {
        if let Some(state) = handle.try_state::<AppState>() {
            let mut is_recording = state.is_recording.lock().await;
            *is_recording = false;
        }

        audio::play_stop_sound();

        println!("[DEBUG] Stopping recording...");
        let audio_path = match audio::stop_recording() {
            Ok(path) => {
                println!("[DEBUG] Recording saved to: {}", path);
                path
            }
            Err(e) => {
                eprintln!("[DEBUG] Failed to stop recording: {}", e);
                crate::tray::set_recording_state(&handle, false);
                return;
            }
        };

        crate::tray::set_recording_state(&handle, false);
        let _ = handle.emit("transcription-started", ());

        println!("[DEBUG] Starting transcription...");
        match sidecar::transcribe(&handle, &audio_path).await {
            Ok(text) => {
                println!("[DEBUG] Transcription result: '{}'", text);
                if !text.is_empty() {
                    let text_clone = text.clone();
                    let keycodes = modifier_keycodes.clone();
                    let result = handle.run_on_main_thread(move || {
                        if let Err(e) = text_insertion::insert_text_via_clipboard(&text_clone, &keycodes) {
                            eprintln!("[DEBUG] Failed to insert text: {}", e);
                        }
                    });

                    if let Err(e) = result {
                        eprintln!("[DEBUG] Failed to run on main thread: {}", e);
                    }
                    let _ = handle.emit("transcription-complete", text);
                } else {
                    println!("[DEBUG] Transcription was empty");
                }
            }
            Err(e) => {
                eprintln!("[DEBUG] Transcription failed: {}", e);
                let _ = handle.emit("transcription-error", e);
            }
        }

        let _ = std::fs::remove_file(&audio_path);
    });
}
