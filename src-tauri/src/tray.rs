use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Manager,
};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref TRAY_ICON: Mutex<Option<TrayIcon>> = Mutex::new(None);
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Saytype", true, None::<&str>)?;

    // Create menu
    let menu = Menu::with_items(app, &[&settings, &quit])?;

    // Create tray icon
    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("Saytype - Push to talk")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "settings" => {
                    // Show settings window
                    if let Some(window) = app.get_webview_window("settings") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    // Store reference
    let mut tray_guard = TRAY_ICON.lock().unwrap();
    *tray_guard = Some(tray);

    Ok(())
}

pub fn set_recording_state(app: &AppHandle, is_recording: bool) {
    let tray_guard = TRAY_ICON.lock().unwrap();
    if let Some(ref tray) = *tray_guard {
        // Swap icon based on recording state
        let icon_name = if is_recording {
            "icons/recording.png"
        } else {
            "icons/32x32.png"
        };

        // Try to load icon - check multiple possible locations
        let icon_loaded = try_load_icon(app, tray, icon_name);

        // If icon loading failed, log it but continue
        if !icon_loaded {
            eprintln!("[TRAY] Failed to load icon: {}", icon_name);
        }

        // Update tooltip
        let tooltip = if is_recording {
            "Saytype - Recording..."
        } else {
            "Saytype - Push to talk"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn try_load_icon(app: &AppHandle, tray: &TrayIcon, icon_name: &str) -> bool {
    // Try resource directory first (production)
    if let Ok(resource_path) = app.path().resource_dir() {
        let icon_path = resource_path.join(icon_name);
        if let Ok(icon) = tauri::image::Image::from_path(&icon_path) {
            let _ = tray.set_icon(Some(icon));
            return true;
        }
    }

    // Try relative to src-tauri directory (development)
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(icon_name);
    if let Ok(icon) = tauri::image::Image::from_path(&dev_path) {
        let _ = tray.set_icon(Some(icon));
        return true;
    }

    false
}
