//! Recent dictations with their audio, so a misheard word can be fixed later and the
//! recording can teach the model the user's own voice.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use tauri::Emitter;

use crate::{audio, config, storage};

const LIMIT: usize = 50;

/// A dictation finishing can land while the History page edits the index
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub at_ms: u64,
    pub text: String,
}

#[derive(Serialize)]
pub struct History {
    keep: bool,
    entries: Vec<Entry>,
}

fn dir() -> Result<PathBuf, String> {
    Ok(config::data_dir()?.join("history"))
}

fn index(dir: &Path) -> PathBuf {
    dir.join("history.json")
}

fn audio_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.wav"))
}

fn entries(dir: &Path) -> Vec<Entry> {
    storage::read_json(&index(dir))
}

fn add(dir: &Path, audio: &Path, text: &str) -> Result<(), String> {
    let entry = Entry {
        id: storage::new_id(),
        at_ms: storage::now_millis(),
        text: text.to_string(),
    };
    storage::move_file(audio, &audio_path(dir, &entry.id))?;
    let mut entries = entries(dir);
    entries.insert(0, entry);
    for dropped in entries.drain(LIMIT.min(entries.len())..) {
        let _ = std::fs::remove_file(audio_path(dir, &dropped.id));
    }
    storage::write_json(&index(dir), &entries)
}

fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let mut entries = entries(dir);
    let before = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == before {
        return Err("That dictation is no longer in your history".to_string());
    }
    let _ = std::fs::remove_file(audio_path(dir, id));
    storage::write_json(&index(dir), &entries)
}

fn clear(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("Failed to clear history: {e}"))
        }
        _ => Ok(()),
    }
}

/// Takes over a finished dictation's recording, unless the user turned history off
pub fn keep(app_handle: &tauri::AppHandle, audio: &Path, text: &str) {
    let _guard = lock();
    if config::load_config().history_off {
        return;
    }
    match dir().and_then(|dir| add(&dir, audio, text)) {
        Ok(()) => {
            let _ = app_handle.emit("history-changed", ());
        }
        Err(e) => eprintln!("[HISTORY] Failed to keep dictation: {e}"),
    }
}

/// Ids come from the frontend, so only ones in the index become paths
fn known_recording(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if entries(dir).iter().any(|entry| entry.id == id) {
        Ok(audio_path(dir, id))
    } else {
        Err("That dictation is no longer in your history".to_string())
    }
}

/// Under the lock, so a dictation landing meanwhile can't evict the entry mid-copy
pub fn copy_recording(id: &str, to: &Path) -> Result<(), String> {
    let dir = dir()?;
    let _guard = lock();
    std::fs::copy(known_recording(&dir, id)?, to)
        .map(|_| ())
        .map_err(|e| format!("Couldn't copy the recording: {e}"))
}

pub fn set_text(id: &str, text: &str) -> Result<(), String> {
    let dir = dir()?;
    let _guard = lock();
    let mut entries = entries(&dir);
    if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
        entry.text = text.to_string();
    }
    storage::write_json(&index(&dir), &entries)
}

#[tauri::command]
pub fn get_history() -> Result<History, String> {
    let dir = dir()?;
    let _guard = lock();
    Ok(History {
        keep: !config::load_config().history_off,
        entries: entries(&dir),
    })
}

#[tauri::command]
pub fn set_keep_history(keep: bool) -> Result<(), String> {
    let dir = dir()?;
    let _guard = lock();
    let mut config = config::load_config();
    config.history_off = !keep;
    config::save_config(&config)?;
    if keep {
        Ok(())
    } else {
        clear(&dir)
    }
}

#[tauri::command]
pub fn delete_history_entry(id: String) -> Result<(), String> {
    let dir = dir()?;
    let _guard = lock();
    remove(&dir, &id)
}

#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    let dir = dir()?;
    let _guard = lock();
    clear(&dir)
}

#[tauri::command]
pub fn play_history_entry(id: String) -> Result<(), String> {
    let dir = dir()?;
    let audio = {
        let _guard = lock();
        known_recording(&dir, &id)?
    };
    audio::play_file(&audio)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("saytype-history-{}", storage::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn recording(dir: &Path) -> PathBuf {
        let path = dir.join(format!("{}.wav", storage::new_id()));
        std::fs::write(&path, b"wav").unwrap();
        path
    }

    #[test]
    fn keeps_the_newest_and_deletes_audio_past_the_limit() {
        let dir = temp_dir();
        let store = dir.join("history");
        for i in 0..LIMIT + 2 {
            add(&store, &recording(&dir), &format!("dictation {i}")).unwrap();
        }

        let kept = entries(&store);
        assert_eq!(kept.len(), LIMIT);
        assert_eq!(kept[0].text, format!("dictation {}", LIMIT + 1));
        assert_eq!(kept[LIMIT - 1].text, "dictation 2");
        let wavs = std::fs::read_dir(&store)
            .unwrap()
            .filter(|f| f.as_ref().unwrap().path().extension().unwrap() == "wav")
            .count();
        assert_eq!(wavs, LIMIT);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remove_deletes_entry_and_audio() {
        let dir = temp_dir();
        let store = dir.join("history");
        add(&store, &recording(&dir), "first").unwrap();
        add(&store, &recording(&dir), "second").unwrap();
        let id = entries(&store)[1].id.clone();

        remove(&store, &id).unwrap();

        assert_eq!(entries(&store).len(), 1);
        assert!(!audio_path(&store, &id).exists());
        assert!(remove(&store, &id).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clear_tolerates_a_missing_directory() {
        let dir = temp_dir();
        assert!(clear(&dir.join("never-created")).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
