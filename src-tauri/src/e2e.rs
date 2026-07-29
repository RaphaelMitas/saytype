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
//! Env vars:
//!   `SAYTYPE_E2E_AUDIO`         WAV to transcribe; presence enables the harness
//!   `SAYTYPE_E2E_EXPECT`        optional expected transcript; mismatch exits 1
//!   `SAYTYPE_E2E_MODE`          local | client | server; defaults to local
//!   `SAYTYPE_E2E_TIMEOUT_SECS`  watchdog, default 900

use std::time::Duration;

use crate::config::{AppConfig, AppMode};

const AUDIO_VAR: &str = "SAYTYPE_E2E_AUDIO";
const EXPECT_VAR: &str = "SAYTYPE_E2E_EXPECT";
const MODE_VAR: &str = "SAYTYPE_E2E_MODE";
const TIMEOUT_VAR: &str = "SAYTYPE_E2E_TIMEOUT_SECS";
const DEFAULT_TIMEOUT_SECS: u64 = 900;

/// The audio file to transcribe, if the harness is enabled.
pub fn audio_path() -> Option<String> {
    std::env::var(AUDIO_VAR).ok().filter(|p| !p.is_empty())
}

pub fn is_enabled() -> bool {
    audio_path().is_some()
}

/// Pins the transcription mode during an E2E run so the result does not depend
/// on whatever config the developer or runner happens to have on disk. Defaults
/// to Local, the mode CI cares about; a no-op outside E2E runs.
pub fn apply_mode_override(mut config: AppConfig) -> AppConfig {
    if !is_enabled() {
        return config;
    }

    let requested = std::env::var(MODE_VAR).unwrap_or_else(|_| "local".to_string());
    config.mode = match requested.to_lowercase().as_str() {
        "local" => AppMode::Local,
        "client" | "client_only" | "clientonly" => AppMode::ClientOnly,
        "server" | "server_only" | "serveronly" => AppMode::ServerOnly,
        other => {
            eprintln!("[E2E] FAIL: unknown {}={:?}", MODE_VAR, other);
            std::process::exit(1);
        }
    };
    println!("[E2E] Forcing transcription mode to {:?}", config.mode);
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
fn finish(code: i32) -> ! {
    crate::sidecar::shutdown();
    std::process::exit(code)
}

/// Transcribes the configured file and exits with 0 on success, 1 on failure.
pub async fn run(app_handle: &tauri::AppHandle, audio_path: &str) -> ! {
    println!("[E2E] Transcribing {}", audio_path);

    if !std::path::Path::new(audio_path).exists() {
        eprintln!("[E2E] FAIL: audio file not found: {}", audio_path);
        finish(1);
    }

    let text = match crate::transcriber::transcribe(app_handle, audio_path).await {
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
    fn normalize_distinguishes_different_words() {
        assert_ne!(normalize("hello world"), normalize("hello there"));
    }
}
