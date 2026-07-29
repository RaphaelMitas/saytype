//! Headless end-to-end harness for CI.
//!
//! When `SAYTYPE_E2E_AUDIO` points at a WAV file, the app skips the parts that
//! need TCC permissions or a user (event tap, settings window), transcribes that
//! file through the normal [`crate::transcriber`] path, and exits.
//!
//! This is the only way to exercise the real bundled app in a pipeline: it covers
//! sidecar path resolution inside `Saytype.app`, process spawn, the ready
//! handshake, and the full Rust -> Python -> Rust round trip — none of which the
//! sidecar-only tests can see.
//!
//! Beware: this gate is nothing but the env var. A stale `SAYTYPE_E2E_AUDIO`
//! export in a dev shell turns any launch from that shell (including
//! `pnpm tauri dev`) into a headless transcribe-and-exit run. It cannot be a
//! compile-time gate because CI must drive the release bundle that ships.
//!
//! Env vars:
//!   `SAYTYPE_E2E_AUDIO`         WAV to transcribe; presence enables the harness
//!   `SAYTYPE_E2E_EXPECT`        optional expected transcript; mismatch exits 1
//!   `SAYTYPE_E2E_MODE`          local | client | server; defaults to local.
//!                               server transcribes through the app's own HTTP
//!                               server; client needs `server_url` in the config.
//!   `SAYTYPE_E2E_TIMEOUT_SECS`  watchdog, default 900

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::config::{AppConfig, AppMode};

const AUDIO_VAR: &str = "SAYTYPE_E2E_AUDIO";
const EXPECT_VAR: &str = "SAYTYPE_E2E_EXPECT";
const MODE_VAR: &str = "SAYTYPE_E2E_MODE";
const TIMEOUT_VAR: &str = "SAYTYPE_E2E_TIMEOUT_SECS";
const DEFAULT_TIMEOUT_SECS: u64 = 900;

/// PIDs of transcribe-server children spawned during this run, so `finish` can
/// guarantee cleanup even when the graceful path is unavailable (wedged sidecar,
/// failed handshake, HTTP-mode child that no module tracks).
static CHILD_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// The audio file to transcribe, if the harness is enabled.
pub fn audio_path() -> Option<String> {
    std::env::var(AUDIO_VAR).ok().filter(|p| !p.is_empty())
}

pub fn is_enabled() -> bool {
    audio_path().is_some()
}

/// Records a spawned sidecar child for kill-on-exit. No-op outside E2E runs.
pub fn register_child(pid: u32) {
    if !is_enabled() {
        return;
    }
    if let Ok(mut pids) = CHILD_PIDS.lock() {
        pids.push(pid);
    }
}

fn kill_registered_children() {
    let Ok(pids) = CHILD_PIDS.lock() else {
        return;
    };
    for pid in pids.iter() {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}

/// Pins the transcription mode during an E2E run so the result does not depend
/// on whatever config the developer or runner happens to have on disk. Defaults
/// to Local, the mode CI cares about; a no-op outside E2E runs.
///
/// Resolved (and logged) once — load_config runs on every transcription and
/// would otherwise spam the E2E output.
pub fn apply_mode_override(mut config: AppConfig) -> AppConfig {
    static OVERRIDE: OnceLock<Option<AppMode>> = OnceLock::new();

    let mode = OVERRIDE.get_or_init(|| {
        if !is_enabled() {
            return None;
        }
        let requested = std::env::var(MODE_VAR).unwrap_or_else(|_| "local".to_string());
        let mode = match requested.to_lowercase().as_str() {
            "local" => AppMode::Local,
            "client" | "client_only" | "clientonly" => AppMode::ClientOnly,
            "server" | "server_only" | "serveronly" => AppMode::ServerOnly,
            other => {
                eprintln!("[E2E] FAIL: unknown {}={:?}", MODE_VAR, other);
                std::process::exit(1);
            }
        };
        println!("[E2E] Forcing transcription mode to {:?}", mode);
        Some(mode)
    });

    if let Some(mode) = mode {
        config.mode = *mode;
    }
    config
}

/// Kills the app if it wedges, so a stuck sidecar fails the job instead of
/// hanging it until the runner's own timeout.
pub fn spawn_watchdog() {
    let secs = std::env::var(TIMEOUT_VAR)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(secs));
        eprintln!("[E2E] FAIL: timed out after {}s", secs);
        finish(1);
    });
}

/// Casing and punctuation are not part of the contract; words are.
/// Keep in sync with `normalize` in tests/e2e/sidecar_e2e.py.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shuts the sidecar down and exits. Never returns.
///
/// `sidecar::shutdown` only try-locks (a wedged transcribe holds that lock
/// across a blocking read — the watchdog must not deadlock behind it), so the
/// PID sweep is what guarantees no orphaned sidecar survives the exit.
pub(crate) fn finish(code: i32) -> ! {
    crate::sidecar::shutdown();
    kill_registered_children();
    std::process::exit(code)
}

/// Transcribes the configured file and exits with 0 on success, 1 on failure.
pub async fn run(app_handle: &tauri::AppHandle, audio_path: &str) -> ! {
    println!("[E2E] Transcribing {}", audio_path);

    if !std::path::Path::new(audio_path).exists() {
        eprintln!("[E2E] FAIL: audio file not found: {}", audio_path);
        finish(1);
    }

    let config = crate::config::load_config();
    let result = match config.mode {
        // In server mode the app only serves HTTP, so drive it the way a client
        // machine would; transcriber::transcribe would refuse ("recording
        // disabled") without ever touching the running server.
        AppMode::ServerOnly => {
            let port = config.server_port.unwrap_or(8765);
            let url = format!("http://127.0.0.1:{}", port);
            println!("[E2E] Server mode: transcribing via {}", url);
            crate::remote::transcribe(&url, audio_path).await
        }
        _ => crate::transcriber::transcribe(app_handle, audio_path).await,
    };

    let text = match result {
        Ok(text) => text,
        Err(e) => {
            eprintln!("[E2E] FAIL: transcription error: {}", e);
            finish(1);
        }
    };

    println!("[E2E] TRANSCRIPT: {}", text);

    if let Ok(expected) = std::env::var(EXPECT_VAR) {
        if normalize(&text) != normalize(&expected) {
            eprintln!("[E2E] FAIL: expected {:?}, got {:?}", expected, text);
            finish(1);
        }
        println!("[E2E] Transcript matches expected text.");
    }

    println!("[E2E] PASS");
    finish(0)
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn normalize_ignores_case_and_punctuation() {
        assert_eq!(
            normalize("The quick brown fox jumps over the lazy dog."),
            normalize("the quick brown fox jumps over the lazy dog")
        );
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  hello   world \n"), "hello world");
    }

    #[test]
    fn normalize_splits_punctuation_joined_words() {
        assert_eq!(normalize("twenty-one"), "twenty one");
        assert_eq!(normalize("don't"), "don t");
    }

    #[test]
    fn normalize_distinguishes_different_words() {
        assert_ne!(normalize("hello world"), normalize("hello there"));
    }
}
