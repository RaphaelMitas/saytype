//! Display only: the paste still comes from the single pass on release.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::config::{self, PreviewStyle};
use crate::{audio, transcriber, AppState};

const LABEL: &str = "overlay";
const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 180.0;
const BOTTOM_MARGIN: f64 = 16.0;
const PREVIEW_INTERVAL: Duration = Duration::from_millis(500);
const LEVEL_INTERVAL: Duration = Duration::from_millis(60);
const LINGER: Duration = Duration::from_millis(1200);
const FADE: Duration = Duration::from_millis(200);

/// Bumped on every press so loops and hide timers from an earlier recording stand down.
static SESSION: AtomicU64 = AtomicU64::new(0);
/// The session still recording, 0 once the hotkey is released.
static LIVE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum OverlayEvent {
    Listening { style: PreviewStyle, hotkey: String },
    Live { text: String },
    Finishing,
    Pasted { text: String },
    Empty,
    Failed { message: String },
    Hidden,
}

pub enum Outcome {
    Pasted(String),
    Empty,
    Failed(String),
}

pub fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Saytype Preview")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    window.set_ignore_cursor_events(true)?;
    #[cfg(target_os = "macos")]
    join_fullscreen_spaces(&window)?;
    Ok(())
}

/// Without this a background app's window stays off another app's fullscreen Space.
#[cfg(target_os = "macos")]
fn join_fullscreen_spaces(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let ns_window = window.ns_window()?.cast::<NSWindow>();
    // SAFETY: Tauri owns the NSWindow for the window's lifetime, and setup runs on the main thread
    let ns_window = unsafe { &*ns_window };
    ns_window.setCollectionBehavior(
        ns_window.collectionBehavior() | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    Ok(())
}

pub fn start(app: &AppHandle) {
    let style = config::load_config().preview;
    if style == PreviewStyle::Off {
        return;
    }
    let session = SESSION.fetch_add(1, Ordering::SeqCst) + 1;
    LIVE.store(session, Ordering::SeqCst);

    let hotkey = app
        .try_state::<AppState>()
        .and_then(|state| state.current_hotkey.lock().ok().map(|h| h.label.clone()))
        .unwrap_or_default();
    show_on_cursor_screen(app);
    send(app, OverlayEvent::Listening { style, hotkey });

    tauri::async_runtime::spawn(level_loop(app.clone(), session));
    tauri::async_runtime::spawn(preview_loop(app.clone(), session));
}

/// Hand the returned session to [`finish`].
pub fn stop(app: &AppHandle) -> u64 {
    let session = LIVE.swap(0, Ordering::SeqCst);
    if session != 0 {
        send(app, OverlayEvent::Finishing);
    }
    session
}

pub fn finish(app: &AppHandle, session: u64, outcome: Outcome) {
    if session == 0 || SESSION.load(Ordering::SeqCst) != session {
        return;
    }
    send(
        app,
        match outcome {
            Outcome::Pasted(text) => OverlayEvent::Pasted { text },
            Outcome::Empty => OverlayEvent::Empty,
            Outcome::Failed(message) => OverlayEvent::Failed { message },
        },
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(LINGER).await;
        if SESSION.load(Ordering::SeqCst) != session {
            return;
        }
        send(&app, OverlayEvent::Hidden);
        tokio::time::sleep(FADE).await;
        if SESSION.load(Ordering::SeqCst) == session {
            if let Some(window) = app.get_webview_window(LABEL) {
                let _ = window.hide();
            }
        }
    });
}

async fn level_loop(app: AppHandle, session: u64) {
    while LIVE.load(Ordering::SeqCst) == session {
        if let Some(level) = audio::recording_level() {
            let _ = app.emit_to(LABEL, "overlay-level", level);
        }
        tokio::time::sleep(LEVEL_INTERVAL).await;
    }
}

async fn preview_loop(app: AppHandle, session: u64) {
    let path = std::env::temp_dir()
        .join(format!(
            "saytype_preview_{}_{}.wav",
            std::process::id(),
            session
        ))
        .to_string_lossy()
        .into_owned();

    let mut wait = PREVIEW_INTERVAL;
    loop {
        tokio::time::sleep(wait).await;
        if LIVE.load(Ordering::SeqCst) != session {
            break;
        }
        if audio::save_snapshot(&path).is_err() {
            continue;
        }
        let started = Instant::now();
        if let Ok(text) = transcriber::transcribe(&app, &path).await {
            if LIVE.load(Ordering::SeqCst) == session {
                send(&app, OverlayEvent::Live { text });
            }
        }
        // A release can't interrupt a pass, so keep the model idle at least two thirds of the time
        wait = PREVIEW_INTERVAL.max(started.elapsed() * 2);
    }

    let _ = std::fs::remove_file(&path);
}

fn send(app: &AppHandle, event: OverlayEvent) {
    let _ = app.emit_to(LABEL, "overlay", event);
}

fn show_on_cursor_screen(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if let Some(monitor) = cursor_monitor(app) {
        // Logical units, because a physical position is converted with the scale of the screen the window is leaving
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let origin = area.position.to_logical::<f64>(scale);
        let size = area.size.to_logical::<f64>(scale);
        let x = origin.x + (size.width - WIDTH) / 2.0;
        let y = origin.y + size.height - HEIGHT - BOTTOM_MARGIN;
        let _ = window.set_size(LogicalSize::new(WIDTH, HEIGHT));
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
    let _ = window.show();
}

fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    let primary = app.primary_monitor().ok().flatten();
    let cursor = app.cursor_position().ok()?;
    // macOS reports the cursor in points scaled by the primary screen, but looks monitors up in points
    #[cfg(target_os = "macos")]
    let cursor = cursor.to_logical::<f64>(primary.as_ref().map_or(1.0, Monitor::scale_factor));
    app.monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or(primary)
}
