use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

use crate::preview::{self, Outcome};
use crate::{audio, history, text_insertion, transcriber, vocab, AppState};

static HOTKEY_ACTIVE: AtomicBool = AtomicBool::new(false);

pub(super) fn set_hotkey_held(app_handle: &tauri::AppHandle, held: bool) {
    if HOTKEY_ACTIVE.swap(held, Ordering::SeqCst) != held {
        if held {
            on_hotkey_pressed(app_handle);
        } else {
            on_hotkey_released(app_handle);
        }
    }
}

fn on_hotkey_pressed(app_handle: &tauri::AppHandle) {
    println!("[DEBUG] Hotkey pressed - starting recording");
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // Without access macOS records silence instead of failing.
        if audio::microphone_permission() != audio::MicPermission::Granted {
            audio::play_busy_sound();
            crate::tray::show_settings(&handle);
            let _ = handle.emit("permission-required", ());
            return;
        }

        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };
        if !*state.sidecar_ready.lock().await {
            println!("[DEBUG] Hotkey pressed but sidecar not ready");
            audio::play_busy_sound();
            return;
        }

        // Held until the overlay is up, so a quick release waits for the press to finish
        let mut is_recording = state.is_recording.lock().await;
        if !HOTKEY_ACTIVE.load(Ordering::SeqCst) {
            return;
        }
        // Checked under the lock, which start_example_recording also holds while it starts
        if vocab::is_recording_example() {
            audio::play_busy_sound();
            return;
        }

        audio::play_start_sound();

        if let Err(e) = audio::start_recording() {
            eprintln!("[DEBUG] Failed to start recording: {}", e);
            return;
        }
        *is_recording = true;

        println!("[DEBUG] Recording started successfully");
        crate::tray::set_recording_state(&handle, true);
        let _ = handle.emit("recording-started", ());
        preview::start(&handle);
    });
}

fn on_hotkey_released(app_handle: &tauri::AppHandle) {
    let handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        // The press was refused, and stopping now would cut off the example
        if vocab::is_recording_example() {
            return;
        }
        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };
        // Held until the recorder stops, so a quick second press can't start in between
        let mut is_recording = state.is_recording.lock().await;
        let was_recording = std::mem::replace(&mut *is_recording, false);
        // A refused press never started recording; its busy sound already played.
        if was_recording {
            audio::play_stop_sound();
        }

        let preview_session = preview::stop(&handle);

        println!("[DEBUG] Stopping recording...");
        let audio_path = audio::stop_recording();
        drop(is_recording);
        crate::tray::set_recording_state(&handle, false);
        let audio_path = match audio_path {
            Ok(path) => {
                println!("[DEBUG] Recording saved to: {}", path);
                path
            }
            Err(e) => {
                eprintln!("[DEBUG] Failed to stop recording: {}", e);
                preview::finish(&handle, preview_session, Outcome::Empty);
                return;
            }
        };

        let _ = handle.emit("transcription-started", ());

        println!("[DEBUG] Starting transcription...");
        match transcriber::transcribe(&handle, &audio_path).await {
            Ok(text) => {
                println!("[DEBUG] Transcription result: '{}'", text);
                if !text.is_empty() {
                    // Before the paste starts, so a fast failure always lands after it
                    preview::finish(&handle, preview_session, Outcome::Pasted(text.clone()));
                    let paste_handle = handle.clone();
                    let paste_text = text.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = text_insertion::insert_text(&paste_handle, &paste_text) {
                            eprintln!("[DEBUG] Failed to insert text: {}", e);
                            preview::finish(&paste_handle, preview_session, Outcome::Failed(e));
                        }
                    });
                    history::keep(&handle, std::path::Path::new(&audio_path), &text);
                    let _ = handle.emit("transcription-complete", text);
                } else {
                    println!("[DEBUG] Transcription was empty");
                    preview::finish(&handle, preview_session, Outcome::Empty);
                }
            }
            Err(e) => {
                eprintln!("[DEBUG] Transcription failed: {}", e);
                preview::finish(&handle, preview_session, Outcome::Failed(e.clone()));
                let _ = handle.emit("transcription-error", e);
            }
        }

        let _ = std::fs::remove_file(&audio_path);
    });
}
