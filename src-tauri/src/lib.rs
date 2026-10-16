#[cfg(target_os = "macos")]
mod app_nap;
mod audio;
mod config;
mod e2e;
mod history;
mod hotkey;
mod preview;
mod remote;
mod sidecar;
mod storage;
mod text_insertion;
mod transcriber;
mod tray;
mod updater;
mod vocab;

use std::sync::Arc;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
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
async fn get_sidecar_ready(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let sidecar_ready = state.sidecar_ready.lock().await;
    Ok(*sidecar_ready)
}

#[tauri::command]
async fn check_permissions() -> Result<serde_json::Value, String> {
    let mic_permission = audio::microphone_permission();
    let accessibility_permission = text_insertion::check_accessibility_permission();
    let input_monitoring_permission = text_insertion::check_input_monitoring_permission();

    Ok(serde_json::json!({
        "microphone": mic_permission,
        "accessibility": accessibility_permission,
        "input_monitoring": input_monitoring_permission
    }))
}

#[tauri::command]
async fn request_microphone_permission() -> audio::MicPermission {
    audio::request_microphone_permission().await
}

#[tauri::command]
async fn open_privacy_settings(pane: text_insertion::PrivacyPane) -> Result<(), String> {
    text_insertion::open_privacy_settings(pane)
}

#[tauri::command]
async fn test_start_recording() -> Result<(), String> {
    println!("[TEST] Starting recording...");
    if audio::microphone_permission() != audio::MicPermission::Granted {
        return Err("Microphone access is off".to_string());
    }
    audio::start_recording()
}

#[tauri::command]
async fn test_stop_and_transcribe(app_handle: tauri::AppHandle) -> Result<String, String> {
    println!("[TEST] Stopping recording...");
    let audio_path = audio::stop_recording()?;
    println!("[TEST] Audio saved to: {}", audio_path);

    println!("[TEST] Starting transcription...");
    let result = transcriber::transcribe(&app_handle, &audio_path).await?;
    println!("[TEST] Transcription result: {}", result);

    // Clean up
    let _ = std::fs::remove_file(&audio_path);

    Ok(result)
}

#[tauri::command]
async fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

/// Restarts the app. Returns true if restarting, false if in dev mode (manual restart needed).
#[tauri::command]
async fn restart_app(app_handle: tauri::AppHandle) -> Result<bool, String> {
    if cfg!(debug_assertions) {
        // Dev mode: can't restart because the Tauri CLI owns the process lifecycle
        return Ok(false);
    }
    app_handle
        .run_on_main_thread(|| {
            let _ = updater::install_pending();
            relaunch()
        })
        .map_err(|e| e.to_string())?;
    Ok(true)
}

pub(crate) fn app_bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .map(std::path::Path::to_path_buf)
}

pub(crate) fn relaunch() -> ! {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = app_bundle() {
        let _ = std::process::Command::new("sh")
            .args(["-c", "sleep 0.5 && open -n \"$1\"", "sh"])
            .arg(bundle)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy().to_string();
        let _ = std::process::Command::new("cmd")
            .args(["/C", &format!("timeout /t 1 >nul && \"{}\"", exe_str)])
            .spawn();
    }
    std::process::exit(0);
}

#[tauri::command]
fn get_update_status() -> updater::UpdateStatus {
    updater::status()
}

#[tauri::command]
async fn check_for_updates(app_handle: tauri::AppHandle) {
    updater::check_and_download(&app_handle).await;
}

#[tauri::command]
fn install_update(app_handle: tauri::AppHandle) -> Result<(), String> {
    let handle = app_handle.clone();
    app_handle
        .run_on_main_thread(move || updater::install_and_restart(&handle))
        .map_err(|e| e.to_string())
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
    app_handle: tauri::AppHandle,
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
        let keycode =
            js_code_to_keycode(code).ok_or_else(|| format!("Unknown key code: {}", code))?;
        let location = params.locations.get(i).copied().unwrap_or(0);

        keycodes.push(keycode);

        if is_modifier_keycode(keycode) {
            modifier_locations.push((keycode, location));
            // Determine modifier type (platform-specific keycodes)
            #[cfg(not(target_os = "windows"))]
            match keycode {
                54 | 55 => modifiers.push(Modifier::Command),
                56 | 60 => modifiers.push(Modifier::Shift),
                58 | 61 => modifiers.push(Modifier::Option),
                59 | 62 => modifiers.push(Modifier::Control),
                63 => modifiers.push(Modifier::Function),
                _ => {}
            }
            #[cfg(target_os = "windows")]
            match keycode {
                0x5B | 0x5C => modifiers.push(Modifier::Command), // Win keys
                0xA0 | 0xA1 => modifiers.push(Modifier::Shift),
                0xA4 | 0xA5 => modifiers.push(Modifier::Option), // Alt keys
                0xA2 | 0xA3 => modifiers.push(Modifier::Control),
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
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    hotkey::clear_held_keys(&app_handle);

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
    let test_path_buf = std::env::temp_dir().join("saytype_test.wav");
    let test_path = test_path_buf.to_str().ok_or("Invalid temp path")?;

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
        writer
            .write_sample(0i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    println!("[TEST] Created test WAV at {}", test_path);

    // Try to transcribe
    match transcriber::transcribe(&app_handle, test_path).await {
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

#[tauri::command]
async fn get_local_ip() -> Result<String, String> {
    // Connect a UDP socket to a public address to determine the local IP
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.connect("8.8.8.8:80").map_err(|e| e.to_string())?;
    let addr = socket.local_addr().map_err(|e| e.to_string())?;
    Ok(addr.ip().to_string())
}

#[tauri::command]
async fn get_network_config() -> Result<serde_json::Value, String> {
    let config = config::load_config();
    Ok(serde_json::json!({
        "mode": config.mode,
        "server_url": config.server_url,
        "server_port": config.server_port,
    }))
}

#[tauri::command]
async fn set_network_config(
    mode: config::AppMode,
    server_url: Option<String>,
    server_port: Option<u16>,
) -> Result<(), String> {
    let mut config = config::load_config();
    config.mode = mode;
    config.server_url = server_url;
    config.server_port = server_port;
    config::save_config(&config)
}

#[tauri::command]
async fn get_preview_style() -> config::PreviewStyle {
    config::load_config().preview
}

#[tauri::command]
async fn set_preview_style(style: config::PreviewStyle) -> Result<(), String> {
    let mut config = config::load_config();
    config.preview = style;
    config::save_config(&config)
}

#[tauri::command]
async fn test_remote_connection(server_url: String) -> Result<bool, String> {
    remote::check_health(&server_url).await
}

#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load config at startup
    let app_config = config::load_config();
    let app_state = AppState::new(app_config.hotkey);

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .setup(|app| {
            // A regular app's windows can't join another app's fullscreen Space, which hides the preview
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            let app_config = config::load_config();
            let is_server_only = app_config.mode == config::AppMode::ServerOnly;
            // Headless CI run: no permissions, no window, no user to drive it.
            let is_e2e = e2e::is_enabled();

            if is_e2e {
                println!("[E2E] Headless mode — skipping tray, window, and hotkey setup");
                e2e::spawn_watchdog();
            }

            // Initialize system tray
            if is_e2e {
                // Skipped: a status item needs a GUI session the runner may not have.
            } else if is_server_only {
                let port = app_config.server_port.unwrap_or(8765);
                tray::setup_tray_with_tooltip(&app_handle, &format!("Saytype - Server :{}", port))?;
            } else {
                tray::setup_tray(&app_handle)?;
            }

            // Open settings window on launch
            if !is_e2e {
                tray::show_settings(&app_handle);
            }

            if !is_server_only && !is_e2e {
                if let Err(e) = preview::create_window(&app_handle) {
                    eprintln!("[PREVIEW] Failed to create overlay window: {}", e);
                }

                // Check accessibility permission before starting hotkey listener (macOS only)
                #[cfg(target_os = "macos")]
                if !text_insertion::check_accessibility_permission() {
                    println!("[HOTKEY] Accessibility permission not granted. Hotkey listener will not work.");
                    let _ = app_handle.emit("permission-required", ());
                }

                // Disable App Nap to ensure event delivery when backgrounded (macOS only)
                #[cfg(target_os = "macos")]
                if let Err(e) = app_nap::disable_app_nap() {
                    eprintln!("[APP_NAP] Warning: Failed to disable App Nap: {}", e);
                }

                // Set up event tap on main run loop (no separate thread needed)
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    println!("[HOTKEY] Setting up event tap on main run loop...");
                    if let Err(e) = hotkey::setup_event_tap(app_handle.clone()) {
                        eprintln!("[HOTKEY] Failed to set up event tap: {}", e);
                        let _ = app_handle.emit("hotkey-error", e);
                    }
                }
            } else if is_server_only {
                println!("[SETUP] Server-only mode — skipping hotkey and permissions setup");
            }

            updater::start(&app_handle);

            // Initialize transcription backend (local sidecar, remote server, or HTTP server)
            let handle_clone = app_handle.clone();
            let e2e_audio = e2e::audio_path();
            tauri::async_runtime::spawn(async move {
                println!("[TRANSCRIBER] Initializing transcription backend...");
                match transcriber::initialize(&handle_clone).await {
                    // After the sidecar, so the two model loads don't compete at launch
                    Ok(()) => vocab::train_if_needed(&handle_clone),
                    Err(e) => {
                        eprintln!("[TRANSCRIBER] Failed to initialize: {}", e);
                    let _ = handle_clone.emit("sidecar-error", e.clone());
                        if e2e_audio.is_some() {
                            eprintln!("[E2E] FAIL: backend init error: {}", e);
                            e2e::finish(1);
                        }
                    }
                }

                if let Some(audio_path) = e2e_audio {
                    e2e::run(&handle_clone, &audio_path).await;
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing it (keeps app running in menu bar)
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
                vocab::settings_closed(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_sidecar_ready,
            check_permissions,
            request_microphone_permission,
            open_privacy_settings,
            test_start_recording,
            test_stop_and_transcribe,
            test_sidecar,
            quit_app,
            get_current_hotkey,
            set_hotkey,
            get_network_config,
            set_network_config,
            test_remote_connection,
            get_preview_style,
            set_preview_style,
            restart_app,
            get_local_ip,
            get_platform,
            get_update_status,
            check_for_updates,
            install_update,
            vocab::get_words,
            vocab::get_vocab_status,
            vocab::add_word,
            vocab::remove_word,
            vocab::set_say_as,
            vocab::speak_word,
            vocab::start_example_recording,
            vocab::finish_example_recording,
            vocab::cancel_example_recording,
            vocab::delete_example,
            vocab::play_example,
            vocab::teach_from_history,
            vocab::retry_training,
            history::get_history,
            history::set_keep_history,
            history::delete_history_entry,
            history::clear_history,
            history::play_history_entry,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| {
            if let RunEvent::Exit = event {
                vocab::shutdown();
                let _ = updater::install_pending();
            }
        });
}
