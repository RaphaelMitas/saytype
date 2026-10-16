//! Custom words: the list, the user's voice examples of them, and the background training that
//! teaches the model to spell them.
//!
//! Training runs the sidecar binary with `--train` as its own process, so dictation keeps
//! working meanwhile. It writes vocab/head.safetensors, which the serving sidecar swaps in before
//! its next transcription. head.json records what that head was trained on; whenever the words
//! or examples differ from it, the head is out of date and a new run gets scheduled.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;

use crate::config::{self, AppMode, Word};
use crate::{audio, e2e, history, sidecar, storage, AppState};

const MAX_WORDS: usize = 50;
const MAX_WORD_CHARS: usize = 50;
/// Lets a burst of edits become one training run
const DEBOUNCE: Duration = Duration::from_secs(20);
/// Longer clips make a training step too large; matches MAX_EXAMPLE_SECONDS in the trainer
const MAX_EXAMPLE_SECONDS: f32 = 15.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    History,
    Recorded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub id: String,
    pub word: String,
    /// What was said, with the word spelled right
    pub text: String,
    pub source: Source,
}

#[derive(Serialize)]
pub struct WordView {
    word: String,
    say_as: Option<String>,
    /// Whether the head in use was trained on this word as it is now
    learned: bool,
    examples: Vec<Example>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Idle,
    /// Edited; training starts once the edits settle
    Waiting,
    Training {
        progress: f32,
    },
    Failed {
        message: String,
    },
}

struct Trainer {
    status: Status,
    /// Bumped by every edit and run, so stale debounce timers and finished runs stand down
    generation: u64,
    child: Option<Child>,
}

static TRAINER: Mutex<Trainer> = Mutex::new(Trainer {
    status: Status::Idle,
    generation: 0,
    child: None,
});

static RECORDING_EXAMPLE: AtomicBool = AtomicBool::new(false);

pub fn vocab_dir() -> Result<PathBuf, String> {
    Ok(config::data_dir()?.join("vocab"))
}

fn examples_dir(vocab: &Path) -> PathBuf {
    vocab.join("examples")
}

fn examples_index(vocab: &Path) -> PathBuf {
    examples_dir(vocab).join("examples.json")
}

fn example_audio(vocab: &Path, id: &str) -> PathBuf {
    examples_dir(vocab).join(format!("{id}.wav"))
}

fn head_meta(vocab: &Path) -> PathBuf {
    vocab.join("head.json")
}

fn read_examples(vocab: &Path) -> Vec<Example> {
    storage::read_json(&examples_index(vocab))
}

/// Everything a head depends on. The app version is in it because a new sidecar may train
/// differently, and retraining after an update takes seconds.
fn spec(words: &[Word], examples: &[Example]) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "words": words,
        "examples": examples
            .iter()
            .map(|e| json!({ "id": e.id, "text": e.text }))
            .collect::<Vec<_>>(),
    })
}

fn trained_spec(vocab: &Path) -> Option<Value> {
    #[derive(Deserialize, Default)]
    struct Meta {
        spec: Option<Value>,
    }
    storage::read_json::<Meta>(&head_meta(vocab)).spec
}

fn learned_words(trained: Option<&Value>) -> Vec<Word> {
    trained
        .and_then(|spec| serde_json::from_value(spec["words"].clone()).ok())
        .unwrap_or_default()
}

fn views(words: &[Word], examples: &[Example], trained: Option<&Value>) -> Vec<WordView> {
    let learned = learned_words(trained);
    words
        .iter()
        .map(|word| WordView {
            word: word.word.clone(),
            say_as: word.say_as.clone(),
            learned: learned.contains(word),
            examples: examples
                .iter()
                .filter(|e| e.word == word.word)
                .cloned()
                .collect(),
        })
        .collect()
}

fn current_views() -> Result<Vec<WordView>, String> {
    let vocab = vocab_dir()?;
    Ok(views(
        &config::load_config().words,
        &read_examples(&vocab),
        trained_spec(&vocab).as_ref(),
    ))
}

fn clean_word(word: &str, words: &[Word]) -> Result<String, String> {
    let word = word.split_whitespace().collect::<Vec<_>>().join(" ");
    if word.is_empty() {
        return Err("Type a word first".to_string());
    }
    if word.chars().count() > MAX_WORD_CHARS {
        return Err(format!("Words can be at most {MAX_WORD_CHARS} characters"));
    }
    if words
        .iter()
        .any(|w| w.word.to_lowercase() == word.to_lowercase())
    {
        return Err(format!("{word} is already in your words"));
    }
    if words.len() >= MAX_WORDS {
        return Err(format!("You can add up to {MAX_WORDS} words"));
    }
    Ok(word)
}

/// The trained head lives where transcription runs, so a client has nothing to train
fn available() -> bool {
    config::load_config().mode != AppMode::ClientOnly && !e2e::is_enabled()
}

fn set_status(app_handle: &tauri::AppHandle, trainer: &mut Trainer, status: Status) {
    if trainer.status != status {
        trainer.status = status.clone();
        let _ = app_handle.emit("vocab-status", status);
    }
}

fn stop_child(trainer: &mut Trainer) {
    if let Some(mut child) = trainer.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn needs_training(vocab: &Path) -> bool {
    let config = config::load_config();
    let current = spec(&config.words, &read_examples(vocab));
    trained_spec(vocab).as_ref() != Some(&current)
}

/// After any edit: a run in progress is training the old list, so it stops, and a new one
/// starts once the edits settle
fn changed(app_handle: &tauri::AppHandle) {
    let _ = app_handle.emit("words-changed", ());
    if !available() {
        return;
    }
    let Ok(vocab) = vocab_dir() else {
        return;
    };
    let mut trainer = TRAINER.lock().unwrap_or_else(|e| e.into_inner());
    trainer.generation += 1;
    stop_child(&mut trainer);
    if !needs_training(&vocab) {
        set_status(app_handle, &mut trainer, Status::Idle);
        return;
    }
    set_status(app_handle, &mut trainer, Status::Waiting);
    let generation = trainer.generation;
    drop(trainer);

    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DEBOUNCE).await;
        let current = TRAINER.lock().unwrap_or_else(|e| e.into_inner()).generation;
        if current == generation {
            train_if_needed(&handle);
        }
    });
}

/// At launch and when Settings closes: no point waiting out the debounce then
pub fn train_if_needed(app_handle: &tauri::AppHandle) {
    if !available() {
        return;
    }
    let Ok(vocab) = vocab_dir() else {
        return;
    };
    let mut trainer = TRAINER.lock().unwrap_or_else(|e| e.into_inner());
    // Edits stop stale runs, so a run still going is training the current list
    if trainer.child.is_some() {
        return;
    }
    if !needs_training(&vocab) {
        set_status(app_handle, &mut trainer, Status::Idle);
        return;
    }
    if let Err(message) = start(app_handle, &mut trainer, &vocab) {
        set_status(app_handle, &mut trainer, Status::Failed { message });
    }
}

#[derive(Deserialize)]
struct TrainerLine {
    progress: Option<f32>,
    success: Option<bool>,
    error: Option<String>,
}

fn start(app_handle: &tauri::AppHandle, trainer: &mut Trainer, vocab: &Path) -> Result<(), String> {
    stop_child(trainer);
    trainer.generation += 1;
    let generation = trainer.generation;

    let words = config::load_config().words;
    let examples = read_examples(vocab);
    let head = vocab.join("head.safetensors");
    if words.is_empty() {
        let _ = std::fs::remove_file(&head);
        let _ = std::fs::remove_file(head_meta(vocab));
        set_status(app_handle, trainer, Status::Idle);
        return Ok(());
    }

    let job = json!({
        "words": words,
        "examples": examples
            .iter()
            .map(|e| json!({
                "id": e.id,
                "audio": example_audio(vocab, &e.id),
                "text": e.text,
            }))
            .collect::<Vec<_>>(),
        "cache_dir": vocab.join("cache"),
        "head_path": head,
        "meta_path": head_meta(vocab),
        "spec": spec(&words, &examples),
    });
    let job_path = vocab.join("job.json");
    storage::write_json(&job_path, &job)?;

    let program = sidecar::get_sidecar_path(app_handle)?;
    println!("[VOCAB] Training {} words", words.len());
    let mut child = std::process::Command::new(program)
        .arg("--train")
        .arg(&job_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Couldn't start training: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Couldn't read training progress")?;
    trainer.child = Some(child);
    set_status(app_handle, trainer, Status::Training { progress: 0.0 });

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        let mut outcome = Err("Training stopped unexpectedly".to_string());
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(line) = serde_json::from_str::<TrainerLine>(&line) else {
                continue;
            };
            if let Some(success) = line.success {
                outcome = if success {
                    Ok(())
                } else {
                    Err(line.error.unwrap_or_else(|| "Training failed".to_string()))
                };
            } else if let Some(progress) = line.progress {
                let mut trainer = TRAINER.lock().unwrap_or_else(|e| e.into_inner());
                if trainer.generation == generation {
                    set_status(&handle, &mut trainer, Status::Training { progress });
                }
            }
        }

        let mut trainer = TRAINER.lock().unwrap_or_else(|e| e.into_inner());
        if trainer.generation != generation {
            // Superseded: a newer edit or run owns the status now
            return;
        }
        stop_child(&mut trainer);
        match outcome {
            Ok(()) => {
                println!("[VOCAB] Training finished");
                set_status(&handle, &mut trainer, Status::Idle);
            }
            Err(message) => {
                eprintln!("[VOCAB] Training failed: {message}");
                set_status(&handle, &mut trainer, Status::Failed { message });
            }
        }
        drop(trainer);
        let _ = handle.emit("words-changed", ());
    });
    Ok(())
}

pub fn shutdown() {
    let mut trainer = TRAINER.lock().unwrap_or_else(|e| e.into_inner());
    stop_child(&mut trainer);
}

/// The hotkey stays out of the way while the Words page records an example
pub fn is_recording_example() -> bool {
    RECORDING_EXAMPLE.load(Ordering::SeqCst)
}

fn add_word_to_config(word: &str) -> Result<String, String> {
    let mut config = config::load_config();
    let word = clean_word(word, &config.words)?;
    config.words.push(Word {
        word: word.clone(),
        say_as: None,
    });
    config::save_config(&config)?;
    Ok(word)
}

fn add_example(
    vocab: &Path,
    audio: &Path,
    word: &str,
    text: &str,
    source: Source,
) -> Result<(), String> {
    let example = Example {
        id: storage::new_id(),
        word: word.to_string(),
        text: text.trim().to_string(),
        source,
    };
    storage::move_file(audio, &example_audio(vocab, &example.id))?;
    let mut examples = read_examples(vocab);
    examples.push(example);
    storage::write_json(&examples_index(vocab), &examples)
}

#[tauri::command]
pub fn get_words() -> Result<Vec<WordView>, String> {
    current_views()
}

#[tauri::command]
pub fn get_vocab_status() -> Status {
    TRAINER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .status
        .clone()
}

#[tauri::command]
pub fn add_word(app_handle: tauri::AppHandle, word: String) -> Result<Vec<WordView>, String> {
    add_word_to_config(&word)?;
    changed(&app_handle);
    current_views()
}

#[tauri::command]
pub fn remove_word(app_handle: tauri::AppHandle, word: String) -> Result<Vec<WordView>, String> {
    let mut config = config::load_config();
    config.words.retain(|w| w.word != word);
    config::save_config(&config)?;

    let vocab = vocab_dir()?;
    let (gone, kept): (Vec<_>, Vec<_>) = read_examples(&vocab)
        .into_iter()
        .partition(|e| e.word == word);
    for example in gone {
        let _ = std::fs::remove_file(example_audio(&vocab, &example.id));
    }
    storage::write_json(&examples_index(&vocab), &kept)?;
    changed(&app_handle);
    current_views()
}

#[tauri::command]
pub fn set_say_as(
    app_handle: tauri::AppHandle,
    word: String,
    say_as: Option<String>,
) -> Result<Vec<WordView>, String> {
    let say_as = say_as
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty() && *s != word);
    let mut config = config::load_config();
    let entry = config
        .words
        .iter_mut()
        .find(|w| w.word == word)
        .ok_or("That word is no longer in your list")?;
    if entry.say_as == say_as {
        return current_views();
    }
    entry.say_as = say_as;
    config::save_config(&config)?;
    changed(&app_handle);
    current_views()
}

#[tauri::command]
pub fn speak_word(word: String) -> Result<(), String> {
    let config = config::load_config();
    let spoken = config
        .words
        .iter()
        .find(|w| w.word == word)
        .and_then(|w| w.say_as.clone())
        .unwrap_or(word);
    audio::speak(&spoken)
}

#[tauri::command]
pub async fn start_example_recording(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if audio::microphone_permission() != audio::MicPermission::Granted {
        return Err("Microphone access is off".to_string());
    }
    let is_recording = state.is_recording.lock().await;
    if *is_recording || RECORDING_EXAMPLE.swap(true, Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }
    audio::start_recording().inspect_err(|_| RECORDING_EXAMPLE.store(false, Ordering::SeqCst))
}

fn stop_example_recording() -> Result<PathBuf, String> {
    if !RECORDING_EXAMPLE.swap(false, Ordering::SeqCst) {
        return Err("Not recording".to_string());
    }
    audio::stop_recording().map(PathBuf::from)
}

#[tauri::command]
pub fn finish_example_recording(
    app_handle: tauri::AppHandle,
    word: String,
    text: String,
) -> Result<Vec<WordView>, String> {
    let audio = stop_example_recording()?;
    let seconds = storage::wav_seconds(&audio).unwrap_or(0.0);
    if seconds < 0.5 {
        let _ = std::fs::remove_file(&audio);
        return Err("That was too short. Try again and read the whole sentence.".to_string());
    }
    if seconds > MAX_EXAMPLE_SECONDS {
        let _ = std::fs::remove_file(&audio);
        return Err("That was too long. Keep examples to one short sentence.".to_string());
    }
    add_example(&vocab_dir()?, &audio, &word, &text, Source::Recorded)?;
    changed(&app_handle);
    current_views()
}

#[tauri::command]
pub fn cancel_example_recording() {
    if let Ok(audio) = stop_example_recording() {
        let _ = std::fs::remove_file(audio);
    }
}

#[tauri::command]
pub fn delete_example(app_handle: tauri::AppHandle, id: String) -> Result<Vec<WordView>, String> {
    let vocab = vocab_dir()?;
    let mut examples = read_examples(&vocab);
    examples.retain(|e| e.id != id);
    storage::write_json(&examples_index(&vocab), &examples)?;
    let _ = std::fs::remove_file(example_audio(&vocab, &id));
    changed(&app_handle);
    current_views()
}

#[tauri::command]
pub fn play_example(id: String) -> Result<(), String> {
    audio::play_file(&example_audio(&vocab_dir()?, &id))
}

#[derive(Serialize)]
pub struct Taught {
    /// False when no word was marked for learning, or the dictation is too long to train on
    example_saved: bool,
}

#[tauri::command]
pub fn teach_from_history(
    app_handle: tauri::AppHandle,
    id: String,
    text: String,
    words: Vec<String>,
) -> Result<Taught, String> {
    let (_, audio) = history::entry(&id)?;
    let mut config = config::load_config();
    let mut learned = Vec::new();
    for word in &words {
        let existing = config
            .words
            .iter()
            .find(|w| w.word.to_lowercase() == word.trim().to_lowercase());
        match existing {
            Some(existing) => learned.push(existing.word.clone()),
            None => {
                let word = clean_word(word, &config.words)?;
                config.words.push(Word {
                    word: word.clone(),
                    say_as: None,
                });
                learned.push(word);
            }
        }
    }
    config::save_config(&config)?;

    let fits = storage::wav_seconds(&audio).is_some_and(|s| s <= MAX_EXAMPLE_SECONDS);
    let example_saved = fits && !learned.is_empty();
    if let (true, Some(word)) = (example_saved, learned.first()) {
        let copy = std::env::temp_dir().join(format!("saytype-example-{}.wav", storage::new_id()));
        std::fs::copy(&audio, &copy).map_err(|e| format!("Couldn't copy the recording: {e}"))?;
        add_example(&vocab_dir()?, &copy, word, &text, Source::History)?;
    }
    history::set_text(&id, &text)?;
    let _ = app_handle.emit("history-changed", ());
    changed(&app_handle);
    Ok(Taught { example_saved })
}

#[tauri::command]
pub fn retry_training(app_handle: tauri::AppHandle) {
    train_if_needed(&app_handle);
}

/// Only Settings lets the user edit words, so closing it means the edits are done
pub fn settings_closed(app_handle: &tauri::AppHandle) {
    let waiting = matches!(
        TRAINER.lock().unwrap_or_else(|e| e.into_inner()).status,
        Status::Waiting
    );
    if waiting {
        train_if_needed(app_handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(word: &str) -> Word {
        Word {
            word: word.to_string(),
            say_as: None,
        }
    }

    #[test]
    fn clean_word_trims_and_collapses_whitespace() {
        assert_eq!(clean_word("  Mac   Book  ", &[]).unwrap(), "Mac Book");
    }

    #[test]
    fn clean_word_rejects_duplicates_ignoring_case() {
        assert!(clean_word("flatsby", &[word("Flatsby")]).is_err());
    }

    #[test]
    fn clean_word_rejects_empty_long_and_full_lists() {
        assert!(clean_word("   ", &[]).is_err());
        assert!(clean_word(&"a".repeat(MAX_WORD_CHARS + 1), &[]).is_err());
        let full: Vec<Word> = (0..MAX_WORDS).map(|i| word(&format!("w{i}"))).collect();
        assert!(clean_word("Kimeta", &full).is_err());
    }

    #[test]
    fn a_word_is_learned_only_as_it_was_trained() {
        let trained = spec(&[word("Flatsby"), word("Kimeta")], &[]);
        let mut renamed = word("Kimeta");
        renamed.say_as = Some("key metta".to_string());
        let current = [word("Flatsby"), renamed, word("Tauri")];

        let learned: Vec<bool> = views(&current, &[], Some(&trained))
            .iter()
            .map(|v| v.learned)
            .collect();

        assert_eq!(learned, vec![true, false, false]);
    }

    #[test]
    fn spec_changes_with_examples_but_not_their_source() {
        let words = [word("Flatsby")];
        let example = Example {
            id: "1".to_string(),
            word: "Flatsby".to_string(),
            text: "The Flatsby app".to_string(),
            source: Source::History,
        };
        let recorded = Example {
            source: Source::Recorded,
            ..example.clone()
        };
        assert_ne!(
            spec(&words, &[]),
            spec(&words, std::slice::from_ref(&example))
        );
        assert_eq!(spec(&words, &[example]), spec(&words, &[recorded]));
    }
}
