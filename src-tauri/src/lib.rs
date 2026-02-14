mod app_nap;
mod audio;
mod config;
mod hotkey;
mod sidecar;
mod text_insertion;
mod tray;

use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
use tokio::sync::Mutex;

pub use config::HotkeyConfig;

pub struct AppState {
    pub is_recording: Arc<Mutex<bool>>,
    pub sidecar_ready: Arc<Mutex<bool>>,
    pub current_hotkey: Arc<std::sync::Mutex<HotkeyConfig>>,
}

impl AppState {
    pub fn new(hotkey: HotkeyConfig) -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            sidecar_ready: Arc::new(Mutex::new(false)),
            current_hotkey: Arc::new(std::sync::Mutex::new(hotkey)),
        }
    }
}

#[tauri::command]
async fn get_recording_state(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let is_recording = state.is_recording.lock().await;
    Ok(*is_recording)
}

#[tauri::command]
async fn get_sidecar_ready(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let sidecar_ready = state.sidecar_ready.lock().await;
    Ok(*sidecar_ready)
}

#[tauri::command]
async fn check_permissions() -> Result<serde_json::Value, String> {
    let mic_permission = audio::check_microphone_permission();
    let accessibility_permission = text_insertion::check_accessibility_permission();
    let input_monitoring_permission = text_insertion::check_input_monitoring_permission();

    Ok(serde_json::json!({
        "microphone": mic_permission,
        "accessibility": accessibility_permission,
        "input_monitoring": input_monitoring_permission
    }))
}

#[tauri::command]
async fn request_microphone_permission() -> Result<bool, String> {
    audio::request_microphone_permission().await
}

#[tauri::command]
async fn open_accessibility_settings() -> Result<(), String> {
    text_insertion::open_accessibility_settings()
}

#[tauri::command]
async fn open_input_monitoring_settings() -> Result<(), String> {
    text_insertion::open_input_monitoring_settings()
}

#[tauri::command]
async fn transcribe_audio(
    audio_path: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    sidecar::transcribe(&app_handle, &audio_path).await
}

#[tauri::command]
async fn test_start_recording() -> Result<(), String> {
    println!("[TEST] Starting recording...");
    audio::start_recording()
}

#[tauri::command]
async fn test_stop_and_transcribe(app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("[TEST] Stopping recording...");
    let audio_path = audio::stop_recording()?;
    println!("[TEST] Audio saved to: {}", audio_path);

    println!("[TEST] Starting transcription...");
    let result = sidecar::transcribe(&app_handle, &audio_path).await?;
    println!("[TEST] Transcription result: {}", result);

    // Clean up
    let _ = std::fs::remove_file(&audio_path);

    Ok(result)
}

#[tauri::command]
async fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
async fn get_current_hotkey(state: tauri::State<'_, AppState>) -> Result<HotkeyConfig, String> {
    let hotkey = state.current_hotkey.lock().map_err(|e| e.to_string())?;
    Ok(hotkey.clone())
}

#[derive(serde::Deserialize)]
pub struct SetHotkeyParams {
    /// JavaScript event.code values for the keys
    pub codes: Vec<String>,
    /// Keyboard locations (1=left, 2=right) for modifiers
    pub locations: Vec<u32>,
}

#[tauri::command]
async fn set_hotkey(
    params: SetHotkeyParams,
    state: tauri::State<'_, AppState>,
) -> Result<HotkeyConfig, String> {
    use config::{build_label, is_modifier_keycode, js_code_to_keycode, Modifier};

    if params.codes.is_empty() {
        return Err("At least one key is required".to_string());
    }

    // Convert JS codes to keycodes
    let mut keycodes: Vec<i64> = Vec::new();
    let mut modifier_locations: Vec<(i64, u32)> = Vec::new();
    let mut modifiers: Vec<Modifier> = Vec::new();
    let mut non_modifier_key: Option<i64> = None;

    for (i, code) in params.codes.iter().enumerate() {
        let keycode = js_code_to_keycode(code)
            .ok_or_else(|| format!("Unknown key code: {}", code))?;
        let location = params.locations.get(i).copied().unwrap_or(0);

        keycodes.push(keycode);

        if is_modifier_keycode(keycode) {
            modifier_locations.push((keycode, location));
            // Determine modifier type
            match keycode {
                54 | 55 => modifiers.push(Modifier::Command),
                56 | 60 => modifiers.push(Modifier::Shift),
                58 | 61 => modifiers.push(Modifier::Option),
                59 | 62 => modifiers.push(Modifier::Control),
                63 => modifiers.push(Modifier::Function),
                _ => {}
            }
        } else {
            non_modifier_key = Some(keycode);
        }
    }

    let label = build_label(&keycodes);

    let new_hotkey = HotkeyConfig {
        modifiers,
        key: non_modifier_key,
        modifier_locations,
        label,
    };

    // Update the hotkey in state
    {
        let mut hotkey = state.current_hotkey.lock().map_err(|e| e.to_string())?;
        *hotkey = new_hotkey.clone();
    }

    // Clear held keys when config changes
    hotkey::clear_held_keys();

    // Save to config file
    let mut app_config = config::load_config();
    app_config.hotkey = new_hotkey.clone();
    config::save_config(&app_config)?;

    println!("[HOTKEY] Updated hotkey to: {}", new_hotkey.label);

    Ok(new_hotkey)
}

#[tauri::command]
async fn test_sidecar(app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("[TEST] Testing sidecar with sample audio...");

    // Create a simple test - just check if sidecar responds
    // We'll create a tiny silent WAV file for testing
    let test_path = "/tmp/saytype_test.wav";

    // Create a minimal WAV file (silent, 1 second)
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(test_path, spec)
        .map_err(|e| format!("Failed to create test WAV: {}", e))?;

    // Write 1 second of silence
    for _ in 0..16000 {
        writer.write_sample(0i16).map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    println!("[TEST] Created test WAV at {}", test_path);

    // Try to transcribe
    match sidecar::transcribe(&app_handle, test_path).await {
        Ok(text) => {
            let _ = std::fs::remove_file(test_path);
            Ok(format!("Sidecar working! Got: '{}'", text))
        }
        Err(e) => {
            let _ = std::fs::remove_file(test_path);
            Err(format!("Sidecar error: {}", e))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load config at startup
    let app_config = config::load_config();
    let app_state = AppState::new(app_config.hotkey);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize system tray
            tray::setup_tray(&app_handle)?;

            // Open settings window on launch
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Check accessibility permission before starting hotkey listener
            if !text_insertion::check_accessibility_permission() {
                println!("[HOTKEY] Accessibility permission not granted. Hotkey listener will not work.");
                let _ = app_handle.emit("accessibility-required", ());
            }

            // Disable App Nap to ensure event delivery when backgrounded
            if let Err(e) = app_nap::disable_app_nap() {
                eprintln!("[APP_NAP] Warning: Failed to disable App Nap: {}", e);
            }

            // Set up event tap on main run loop (no separate thread needed)
            println!("[HOTKEY] Setting up event tap on main run loop...");
            if let Err(e) = hotkey::setup_event_tap(app_handle.clone()) {
                eprintln!("[HOTKEY] Failed to set up event tap: {}", e);
                let _ = app_handle.emit("hotkey-error", e);
            }

            // Start sidecar process
            let handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                println!("[SIDECAR] Starting sidecar process...");
                if let Err(e) = sidecar::start(&handle_clone).await {
                    eprintln!("[SIDECAR] Failed to start sidecar: {}", e);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing it (keeps app running in menu bar)
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_recording_state,
            get_sidecar_ready,
            check_permissions,
            request_microphone_permission,
            open_accessibility_settings,
            open_input_monitoring_settings,
            transcribe_audio,
            test_start_recording,
            test_stop_and_transcribe,
            test_sidecar,
            quit_app,
            get_current_hotkey,
            set_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
