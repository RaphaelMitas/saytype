use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

// Awake time, not wall time: the clock behind tokio's sleep pauses while the Mac sleeps
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const PROGRESS_STEP_BYTES: u64 = 1_000_000;

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    Disabled,
    Idle,
    Checking,
    UpToDate,
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Ready {
        version: String,
    },
    Error {
        message: String,
    },
}

struct UpdaterState {
    status: UpdateStatus,
    pending: Option<(Update, Vec<u8>)>,
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<UpdaterState> = Mutex::new(UpdaterState {
        status: UpdateStatus::Disabled,
        pending: None,
    });
}

/// Release builds on macOS only: dev builds would replace themselves with the latest release,
/// and releases ship no Windows update feed.
pub fn is_supported() -> bool {
    cfg!(target_os = "macos") && !cfg!(debug_assertions) && !crate::e2e::is_enabled()
}

pub fn status() -> UpdateStatus {
    STATE.lock().unwrap().status.clone()
}

fn set_status(app: &AppHandle, status: UpdateStatus) {
    STATE.lock().unwrap().status = status.clone();
    let _ = app.emit("update-status", status);
}

pub fn start(app: &AppHandle) {
    if !is_supported() {
        return;
    }
    set_status(app, UpdateStatus::Idle);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            check(&app).await;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Checks for an update and downloads it, leaving it ready to install on the next restart.
pub async fn check(app: &AppHandle) {
    {
        let mut state = STATE.lock().unwrap();
        if !matches!(
            state.status,
            UpdateStatus::Idle | UpdateStatus::UpToDate | UpdateStatus::Error { .. }
        ) {
            return;
        }
        state.status = UpdateStatus::Checking;
    }
    let _ = app.emit("update-status", UpdateStatus::Checking);

    match download(app).await {
        Ok(Some((update, bytes))) => {
            let version = update.version.clone();
            STATE.lock().unwrap().pending = Some((update, bytes));
            set_status(
                app,
                UpdateStatus::Ready {
                    version: version.clone(),
                },
            );
            crate::tray::set_update_ready(app, Some(&version));
        }
        Ok(None) => set_status(app, UpdateStatus::UpToDate),
        Err(e) => {
            eprintln!("[UPDATER] {}", e);
            set_status(
                app,
                UpdateStatus::Error {
                    message: e.to_string(),
                },
            );
        }
    }
}

async fn download(app: &AppHandle) -> tauri_plugin_updater::Result<Option<(Update, Vec<u8>)>> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(None);
    };

    let version = update.version.clone();
    let mut downloaded = 0u64;
    let mut reported = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if downloaded - reported >= PROGRESS_STEP_BYTES {
                    reported = downloaded;
                    set_status(
                        app,
                        UpdateStatus::Downloading {
                            version: version.clone(),
                            downloaded,
                            total,
                        },
                    );
                }
            },
            || {},
        )
        .await?;

    Ok(Some((update, bytes)))
}

/// Swaps the app bundle for the downloaded update. Returns false when none is pending.
pub fn install_pending() -> Result<bool, String> {
    // Held for the whole install so a second restart or a quit waits instead of exiting mid-swap
    let mut state = STATE.lock().unwrap();
    let Some((update, bytes)) = state.pending.take() else {
        return Ok(false);
    };
    update.install(bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn install_and_restart(app: &AppHandle) -> Result<(), String> {
    match install_pending() {
        Ok(true) => crate::relaunch(),
        Ok(false) => Ok(()),
        Err(e) => {
            eprintln!("[UPDATER] Install failed: {}", e);
            set_status(app, UpdateStatus::Error { message: e.clone() });
            crate::tray::set_update_ready(app, None);
            Err(e)
        }
    }
}
