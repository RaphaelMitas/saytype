use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager,
};

lazy_static::lazy_static! {
    static ref TRAY_ICON: Mutex<Option<TrayIcon>> = Mutex::new(None);
}

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static UPDATE_READY: AtomicBool = AtomicBool::new(false);

const BADGE_RGBA: [u8; 4] = [0x0a, 0x84, 0xff, 0xff];

pub fn setup_tray_with_tooltip(
    app: &AppHandle,
    tooltip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_tray_inner(app, tooltip)
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    setup_tray_inner(app, "Saytype - Push to talk")
}

fn build_menu(app: &AppHandle, ready_version: Option<&str>) -> tauri::Result<Menu<tauri::Wry>> {
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
    let menu = Menu::new(app)?;
    if let Some(version) = ready_version {
        let label = format!("Restart to Update ({})", version);
        menu.append(&item("install_update", &label)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&item("settings", "Settings...")?)?;
    if ready_version.is_none() && crate::updater::is_supported() {
        menu.append(&item("check_updates", "Check for Updates...")?)?;
    }
    menu.append(&item("quit", "Quit Saytype")?)?;
    Ok(menu)
}

fn setup_tray_inner(app: &AppHandle, tooltip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(app, None)?;

    // Create tray icon
    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip(tooltip)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_settings(app),
            "install_update" => crate::updater::install_and_restart(app),
            "check_updates" => {
                show_settings(app);
                let _ = app.emit("show-about", ());
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::updater::check_and_download(&app).await
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    // Store reference
    let mut tray_guard = TRAY_ICON.lock().unwrap();
    *tray_guard = Some(tray);

    Ok(())
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn set_recording_state(app: &AppHandle, is_recording: bool) {
    IS_RECORDING.store(is_recording, Ordering::Relaxed);
    if let Some(tray) = tray_icon() {
        refresh_icon(app, &tray);

        // Update tooltip
        let tooltip = if is_recording {
            "Saytype - Recording..."
        } else {
            "Saytype - Push to talk"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

// Cloned out so no lock is held while tray calls wait on the main thread
fn tray_icon() -> Option<TrayIcon> {
    TRAY_ICON.lock().unwrap().clone()
}

pub fn set_update_ready(app: &AppHandle, version: Option<&str>) {
    UPDATE_READY.store(version.is_some(), Ordering::Relaxed);
    if let Some(tray) = tray_icon() {
        if let Ok(menu) = build_menu(app, version) {
            let _ = tray.set_menu(Some(menu));
        }
        refresh_icon(app, &tray);
    }
}

fn refresh_icon(app: &AppHandle, tray: &TrayIcon) {
    let icon_name = if IS_RECORDING.load(Ordering::Relaxed) {
        "icons/recording.png"
    } else {
        "icons/32x32.png"
    };
    let Some(icon) = load_icon(app, icon_name) else {
        eprintln!("[TRAY] Failed to load icon: {}", icon_name);
        return;
    };
    let icon = if UPDATE_READY.load(Ordering::Relaxed) {
        with_badge(&icon)
    } else {
        icon
    };
    let _ = tray.set_icon(Some(icon));
}

fn load_icon(app: &AppHandle, icon_name: &str) -> Option<Image<'static>> {
    if let Ok(resource_path) = app.path().resource_dir() {
        if let Ok(icon) = Image::from_path(resource_path.join(icon_name)) {
            return Some(icon);
        }
    }

    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(icon_name);
    Image::from_path(dev_path).ok()
}

fn with_badge(icon: &Image<'_>) -> Image<'static> {
    let (width, height) = (icon.width(), icon.height());
    let mut rgba = icon.rgba().to_vec();
    let radius = width as f32 * 0.2;
    let (cx, cy) = (width as f32 - radius, radius);

    for y in 0..height {
        for x in 0..width {
            let distance = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy);
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            if coverage == 0.0 {
                continue;
            }
            let i = ((y * width + x) * 4) as usize;
            for (channel, target) in BADGE_RGBA.into_iter().enumerate() {
                let current = rgba[i + channel] as f32;
                rgba[i + channel] = (current + (target as f32 - current) * coverage).round() as u8;
            }
        }
    }

    Image::new_owned(rgba, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_covers_top_right_and_leaves_the_rest() {
        let icon = Image::new_owned(vec![0; 32 * 32 * 4], 32, 32);
        let badged = with_badge(&icon);
        let pixel = |x: u32, y: u32| {
            let i = ((y * 32 + x) * 4) as usize;
            badged.rgba()[i..i + 4].to_vec()
        };
        assert_eq!(pixel(25, 6), vec![0x0a, 0x84, 0xff, 255]);
        assert_eq!(pixel(4, 28), vec![0, 0, 0, 0]);
    }
}
