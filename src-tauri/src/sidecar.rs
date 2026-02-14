use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Deserialize)]
struct TranscribeResponse {
    success: bool,
    text: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct StatusResponse {
    status: Option<String>,
}

struct SidecarProcess {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

lazy_static::lazy_static! {
    static ref SIDECAR: Mutex<Option<SidecarProcess>> = Mutex::new(None);
}

fn get_sidecar_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // Sidecar naming convention: name-target_triple (production) or just name (development)
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let sidecar_name_full = "transcribe-server-aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let sidecar_name_full = "transcribe-server-x86_64-apple-darwin";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let sidecar_name_full = "transcribe-server-x86_64-pc-windows-msvc.exe";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let sidecar_name_full = "transcribe-server-x86_64-unknown-linux-gnu";

    let sidecar_name_short = if cfg!(windows) {
        "transcribe-server.exe"
    } else {
        "transcribe-server"
    };

    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?
        .parent()
        .ok_or("No parent dir")?
        .to_path_buf();

    // Try development path first (just the name, no arch suffix)
    let dev_path = exe_dir.join(sidecar_name_short);
    if dev_path.exists() {
        return Ok(dev_path);
    }

    // Try production path (with arch suffix)
    let prod_path = exe_dir.join(sidecar_name_full);
    if prod_path.exists() {
        return Ok(prod_path);
    }

    // Try resource directory
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let resource_path = resource_dir.join(sidecar_name_full);
    if resource_path.exists() {
        return Ok(resource_path);
    }

    Err(format!(
        "Sidecar not found at {:?}, {:?}, or {:?}",
        dev_path, prod_path, resource_path
    ))
}

pub async fn start(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let program = get_sidecar_path(app_handle)?;

    println!("[SIDECAR] Spawning: {:?}", program);

    // Spawn with stdio pipes (inherit stderr so we see Python debug output)
    let mut child = std::process::Command::new(&program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn sidecar: {}", e))?;

    let stdin = child.stdin.take().ok_or("Failed to get sidecar stdin")?;
    let stdout = child.stdout.take().ok_or("Failed to get sidecar stdout")?;

    let mut reader = BufReader::new(stdout);

    // Wait for loading signal
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read from sidecar: {}", e))?;

    let response: StatusResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse status response: {} - got: {}", e, line))?;

    if response.status.as_deref() == Some("loading") {
        println!("[SIDECAR] Model is loading...");
        let _ = app_handle.emit("sidecar-loading", ());

        // Wait for ready signal
        line.clear();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read ready signal from sidecar: {}", e))?;

        let ready_response: StatusResponse = serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse ready response: {} - got: {}", e, line))?;

        if ready_response.status.as_deref() != Some("ready") {
            return Err(format!("Expected 'ready' status, got: {}", line));
        }
    } else if response.status.as_deref() != Some("ready") {
        return Err(format!("Unexpected sidecar response: {}", line));
    }

    // Store the process
    {
        let mut sidecar = SIDECAR.lock().map_err(|e| e.to_string())?;
        *sidecar = Some(SidecarProcess {
            _child: child,
            stdin,
            stdout: reader,
        });
    } // Drop the lock before awaiting

    if let Some(state) = app_handle.try_state::<crate::AppState>() {
        let mut sidecar_ready = state.sidecar_ready.lock().await;
        *sidecar_ready = true;
    }

    let _ = app_handle.emit("sidecar-ready", ());

    println!("Sidecar is ready");
    Ok(())
}

pub async fn transcribe(
    _app_handle: &tauri::AppHandle,
    audio_path: &str,
) -> Result<String, String> {
    let mut sidecar = SIDECAR.lock().map_err(|e| e.to_string())?;
    let process = sidecar.as_mut().ok_or("Sidecar not running")?;

    // Send transcribe command
    let command = serde_json::json!({
        "command": "transcribe",
        "audio_path": audio_path
    });

    writeln!(process.stdin, "{}", command)
        .map_err(|e| format!("Failed to send command to sidecar: {}", e))?;
    process
        .stdin
        .flush()
        .map_err(|e| format!("Failed to flush sidecar stdin: {}", e))?;

    // Read response
    let mut line = String::new();
    process
        .stdout
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read from sidecar: {}", e))?;

    let response: TranscribeResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse transcription response: {} - Raw: {}", e, line))?;

    if response.success {
        response
            .text
            .ok_or_else(|| "No text in response".to_string())
    } else {
        Err(response
            .error
            .unwrap_or_else(|| "Unknown error".to_string()))
    }
}
