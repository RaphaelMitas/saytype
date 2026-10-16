use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        now_millis(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// A missing or unreadable file reads as empty, so a corrupt index never blocks the app
pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create {parent:?}: {e}"))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Failed to write {tmp:?}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Failed to write {path:?}: {e}"))
}

/// Falls back to copying when the temp dir sits on another volume
pub fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create {parent:?}: {e}"))?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| format!("Failed to move {from:?}: {e}"))?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

pub fn wav_seconds(path: &Path) -> Option<f32> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    Some(reader.duration() as f32 / spec.sample_rate as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_within_a_run() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }

    #[test]
    fn unreadable_json_reads_as_default() {
        let dir = std::env::temp_dir().join(format!("saytype-storage-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.json");
        std::fs::write(&path, "{ not json").unwrap();
        let entries: Vec<String> = read_json(&path);
        assert!(entries.is_empty());

        write_json(&path, &vec!["a".to_string()]).unwrap();
        let entries: Vec<String> = read_json(&path);
        assert_eq!(entries, vec!["a"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
