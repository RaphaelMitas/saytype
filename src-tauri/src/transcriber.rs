use std::process::Stdio;
use tauri::{Emitter, Manager};

use crate::config::{AppMode, load_config};
use crate::{remote, sidecar, AppState};

/// Transcribe audio, routing to local sidecar or remote server based on config.
pub async fn transcribe(app_handle: &tauri::AppHandle, audio_path: &str) -> Result<String, String> {
    let config = load_config();
    match config.mode {
        AppMode::Local => sidecar::transcribe(app_handle, audio_path).await,
        AppMode::ClientOnly => {
            let server_url = config
                .server_url
                .as_deref()
                .ok_or("No server URL configured")?;
            remote::transcribe(server_url, audio_path).await
        }
        AppMode::ServerOnly => {
            Err("Server mode — recording disabled".to_string())
        }
    }
}

/// Initialize the transcription backend (local sidecar or remote health poll).
pub async fn initialize(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config = load_config();
    match config.mode {
        AppMode::Local => {
            sidecar::start(app_handle).await
        }
        AppMode::ClientOnly => {
            let server_url = config
                .server_url
                .clone()
                .ok_or("No server URL configured for client mode")?;

            let _ = app_handle.emit("sidecar-loading", ());
            println!("[TRANSCRIBER] Polling remote server at {}...", server_url);

            // Poll until ready (with timeout)
            for i in 0..60 {
                match remote::check_health(&server_url).await {
                    Ok(true) => {
                        println!("[TRANSCRIBER] Remote server is ready");
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let mut ready = state.sidecar_ready.lock().await;
                            *ready = true;
                        }
                        let _ = app_handle.emit("sidecar-ready", ());
                        return Ok(());
                    }
                    Ok(false) => {
                        println!("[TRANSCRIBER] Remote server loading (attempt {}/60)", i + 1);
                    }
                    Err(e) => {
                        println!("[TRANSCRIBER] Health check failed (attempt {}/60): {}", i + 1, e);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }

            Err("Timed out waiting for remote server to become ready".to_string())
        }
        AppMode::ServerOnly => {
            let port = config.server_port.unwrap_or(8765);
            let program = sidecar::get_sidecar_path(app_handle)?;

            // Kill any stale sidecar still bound to this port (Unix only)
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!("lsof -ti :{} | xargs kill -9 2>/dev/null", port))
                    .status();
            }

            println!("[TRANSCRIBER] Starting sidecar in HTTP server mode on port {}...", port);
            let _ = app_handle.emit("sidecar-loading", ());

            let mut child = std::process::Command::new(&program)
                .args(["--http", "--host", "0.0.0.0", "--port", &port.to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| format!("Failed to spawn sidecar in HTTP mode: {}", e))?;

            let stdout = child.stdout.take().ok_or("Failed to get sidecar stdout")?;
            let reader = std::io::BufReader::new(stdout);

            // Wait for "[HTTP] Listening on ..." line on stdout
            use std::io::BufRead;
            let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

            std::thread::spawn(move || {
                let mut tx = Some(tx);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            println!("[SIDECAR-HTTP] {}", l);
                            if tx.is_some() && l.contains("[HTTP] Listening on") {
                                let _ = tx.take().unwrap().send(Ok(()));
                                // Continue draining stdout so the process doesn't block
                            }
                        }
                        Err(e) => {
                            if let Some(tx) = tx.take() {
                                let _ = tx.send(Err(format!("Failed to read sidecar output: {}", e)));
                            }
                            return;
                        }
                    }
                }
                if let Some(tx) = tx.take() {
                    let _ = tx.send(Err("Sidecar process exited before becoming ready".to_string()));
                }
            });

            // Wait with timeout
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                rx,
            ).await
                .map_err(|_| "Timed out waiting for sidecar HTTP server to start".to_string())?
                .map_err(|_| "Channel closed unexpectedly".to_string())??;

            if let Some(state) = app_handle.try_state::<AppState>() {
                let mut ready = state.sidecar_ready.lock().await;
                *ready = true;
            }
            let _ = app_handle.emit("sidecar-ready", ());

            println!("[TRANSCRIBER] Sidecar HTTP server is ready on port {}", port);
            Ok(())
        }
    }
}
