use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
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

struct Pending {
    update: Update,
    archive: PathBuf,
}

struct UpdaterState {
    status: UpdateStatus,
    pending: Option<Pending>,
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<UpdaterState> = Mutex::new(UpdaterState {
        status: UpdateStatus::Disabled,
        pending: None,
    });
}

/// Only the release workflow sets SAYTYPE_UPDATER: a local build would replace itself with the published release.
pub fn is_supported() -> bool {
    option_env!("SAYTYPE_UPDATER").is_some() && !crate::e2e::is_enabled()
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
            check_and_download(&app).await;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

pub async fn check_and_download(app: &AppHandle) {
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
        Ok(Some(pending)) => {
            let version = pending.update.version.clone();
            STATE.lock().unwrap().pending = Some(pending);
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

async fn download(app: &AppHandle) -> Result<Option<Pending>, Box<dyn Error>> {
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

    // On disk rather than in memory: a menu bar app can sit on a ready update for days
    let cache_dir = app.path().app_cache_dir()?;
    tokio::fs::create_dir_all(&cache_dir).await?;
    let archive = cache_dir.join("update.tar.gz");
    tokio::fs::write(&archive, bytes).await?;

    Ok(Some(Pending { update, archive }))
}

/// Main thread only: the plugin asks for an admin password on the main thread when the app
/// isn't writable, so a main thread waiting on an install elsewhere would deadlock.
pub fn install_pending() -> Result<(), String> {
    let Some(Pending { update, archive }) = STATE.lock().unwrap().pending.take() else {
        return Ok(());
    };
    let result = install(&update, &archive).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&archive);
    if let Err(e) = &result {
        eprintln!("[UPDATER] Install failed: {}", e);
    }
    result
}

fn install(update: &Update, archive: &Path) -> Result<(), Box<dyn Error>> {
    // Replaced while running, e.g. by `brew upgrade`: installing would downgrade it
    if installed_version().as_deref() != Some(update.current_version.as_str()) {
        return Ok(());
    }
    update.install(std::fs::read(archive)?)?;
    Ok(())
}

fn installed_version() -> Option<String> {
    #[derive(Deserialize)]
    struct InfoPlist {
        #[serde(rename = "CFBundleShortVersionString")]
        version: String,
    }
    let info_plist = crate::app_bundle()?.join("Contents/Info.plist");
    plist::from_file::<_, InfoPlist>(info_plist)
        .ok()
        .map(|info| info.version)
}

/// Main thread only, see [`install_pending`].
pub fn install_and_restart(app: &AppHandle) {
    if let Err(message) = install_pending() {
        set_status(app, UpdateStatus::Error { message });
        crate::tray::set_update_ready(app, None);
        return;
    }
    crate::relaunch()
}
