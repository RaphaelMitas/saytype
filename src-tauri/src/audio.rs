use audioadapter_buffers::direct::InterleavedSlice;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter};
use rubato::{Fft, FixedSync, Resampler};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

const TARGET_SAMPLE_RATE: u32 = 16000; // 16kHz for Whisper/Parakeet

/// Push-to-talk recorder using native CPAL for audio capture
pub struct PushToTalkRecorder {
    stream: Option<Stream>,
    samples: Arc<StdMutex<Vec<f32>>>,
    sample_rate: u32,
    output_path: String,
}

// SAFETY: Stream is not Send/Sync but we ensure it's only accessed from the thread that created it
// The Mutex ensures exclusive access to the recorder state
unsafe impl Send for PushToTalkRecorder {}

impl PushToTalkRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            samples: Arc::new(StdMutex::new(Vec::new())),
            sample_rate: 0,
            output_path: String::new(),
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        // Generate unique output path
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        self.output_path = std::env::temp_dir()
            .join(format!(
                "saytype_ptt_{}_{}.wav",
                std::process::id(),
                timestamp
            ))
            .to_string_lossy()
            .to_string();

        // Get default input device
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        // Get device's native config
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        self.sample_rate = config.sample_rate();

        // Clear the sample buffer
        {
            let mut samples = self.samples.lock().map_err(|e| e.to_string())?;
            samples.clear();
        }

        // Clone Arc for the callback
        let samples = Arc::clone(&self.samples);

        // Build input stream based on sample format
        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let samples_clone = Arc::clone(&samples);
                device
                    .build_input_stream(
                        config.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut buffer) = samples_clone.lock() {
                                buffer.extend_from_slice(data);
                            }
                        },
                        move |err| {
                            eprintln!("Audio input error: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("Failed to build input stream: {}", e))?
            }
            SampleFormat::I16 => {
                let samples_clone = Arc::clone(&samples);
                device
                    .build_input_stream(
                        config.into(),
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut buffer) = samples_clone.lock() {
                                // Convert i16 to f32
                                for &sample in data {
                                    buffer.push(sample as f32 / i16::MAX as f32);
                                }
                            }
                        },
                        move |err| {
                            eprintln!("Audio input error: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("Failed to build input stream: {}", e))?
            }
            SampleFormat::U16 => {
                let samples_clone = Arc::clone(&samples);
                device
                    .build_input_stream(
                        config.into(),
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut buffer) = samples_clone.lock() {
                                // Convert u16 to f32 (centered at 0)
                                for &sample in data {
                                    let normalized = (sample as f32 - 32768.0) / 32768.0;
                                    buffer.push(normalized);
                                }
                            }
                        },
                        move |err| {
                            eprintln!("Audio input error: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("Failed to build input stream: {}", e))?
            }
            format => {
                return Err(format!("Unsupported sample format: {:?}", format));
            }
        };

        // Start recording
        stream
            .play()
            .map_err(|e| format!("Failed to start recording: {}", e))?;

        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<String, String> {
        // Stop and drop the stream
        if let Some(stream) = self.stream.take() {
            drop(stream);
        } else {
            return Err("No recording in progress".to_string());
        }

        // Small delay to ensure all samples are collected
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Take samples from buffer
        let samples = {
            let mut buffer = self.samples.lock().map_err(|e| e.to_string())?;
            std::mem::take(&mut *buffer)
        };

        if samples.is_empty() {
            return Err("No audio samples recorded".to_string());
        }

        // Resample to 16kHz if needed
        let resampled = if self.sample_rate != TARGET_SAMPLE_RATE {
            resample_audio(&samples, self.sample_rate, TARGET_SAMPLE_RATE)?
        } else {
            samples
        };

        // Write to WAV file
        write_wav_file(&self.output_path, &resampled, TARGET_SAMPLE_RATE)?;

        // Verify file was created
        if !std::path::Path::new(&self.output_path).exists() {
            return Err("WAV file was not created".to_string());
        }

        if let Ok(metadata) = std::fs::metadata(&self.output_path) {
            if metadata.len() <= 44 {
                return Err("WAV file is empty or too small".to_string());
            }
        }

        Ok(self.output_path.clone())
    }
}

#[cfg(test)]
mod resample_tests {
    use super::*;

    #[test]
    fn sine_wave_survives_44100_to_16000() {
        let src_rate = 44100u32;
        let dst_rate = 16000u32;
        let secs = 1.0;
        let n = (src_rate as f32 * secs) as usize;
        let f = 440.0;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * f * i as f32 / src_rate as f32).sin())
            .collect();

        let out = resample_audio(&input, src_rate, dst_rate).unwrap();
        let expected = (n as f32 * dst_rate as f32 / src_rate as f32) as usize;
        assert!(
            (out.len() as i32 - expected as i32).abs() <= 8,
            "len {} vs expected {}",
            out.len(),
            expected
        );

        let (start, end) = (4096, out.len() - 4096);
        let mut crossings = 0u32;
        for w in out[start..end].windows(2) {
            if (w[0] < 0.0 && w[1] >= 0.0) || (w[0] >= 0.0 && w[1] < 0.0) {
                crossings += 1;
            }
        }
        let window_secs = (end - start) as f32 / dst_rate as f32;
        let est_freq = crossings as f32 / 2.0 / window_secs;
        assert!(
            (est_freq - f).abs() < 2.0,
            "estimated freq {} vs expected {}",
            est_freq,
            f
        );
    }
}

/// Resample audio from source_rate to target_rate using rubato
fn resample_audio(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, String> {
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let mut resampler = Fft::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        1024, // chunk_size
        1,    // mono
        FixedSync::Both,
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let input_adapter = InterleavedSlice::new(samples, 1, samples.len())
        .map_err(|e| format!("Failed to create input adapter: {}", e))?;

    let output_len = resampler.process_all_needed_output_len(samples.len());
    let mut output = vec![0.0_f32; output_len];
    let mut output_adapter = InterleavedSlice::new_mut(&mut output, 1, output_len)
        .map_err(|e| format!("Failed to create output adapter: {}", e))?;

    let (_, frames_written) = resampler
        .process_all_into_buffer(&input_adapter, &mut output_adapter, samples.len(), None)
        .map_err(|e| format!("Resampling error: {}", e))?;

    output.truncate(frames_written);
    Ok(output)
}

/// Write samples to a WAV file
fn write_wav_file(path: &str, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV file: {}", e))?;

    for &sample in samples {
        // Convert f32 [-1.0, 1.0] to i16
        let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

// Global recorder using std Mutex for thread-safety
lazy_static::lazy_static! {
    pub static ref PTT_RECORDER: StdMutex<PushToTalkRecorder> = StdMutex::new(PushToTalkRecorder::new());
}

pub fn start_recording() -> Result<(), String> {
    let mut recorder = PTT_RECORDER.lock().map_err(|e| e.to_string())?;
    recorder.start()
}

pub fn stop_recording() -> Result<String, String> {
    let mut recorder = PTT_RECORDER.lock().map_err(|e| e.to_string())?;
    recorder.stop()
}

/// Shares the buffer of a running recording so readers don't hold the recorder lock.
fn live_samples() -> Option<(Arc<StdMutex<Vec<f32>>>, u32)> {
    let recorder = PTT_RECORDER.lock().ok()?;
    recorder.stream.as_ref()?;
    Some((Arc::clone(&recorder.samples), recorder.sample_rate))
}

/// Writes what has been recorded so far to a 16kHz WAV, leaving the recording running.
pub fn save_snapshot(path: &str) -> Result<(), String> {
    let (samples, sample_rate) = live_samples().ok_or("No recording in progress")?;
    let samples = samples.lock().map_err(|e| e.to_string())?.clone();
    if samples.is_empty() {
        return Err("No audio samples recorded".to_string());
    }
    let resampled = resample_audio(&samples, sample_rate, TARGET_SAMPLE_RATE)?;
    write_wav_file(path, &resampled, TARGET_SAMPLE_RATE)
}

/// Loudness of the last 50ms on a 0..1 scale spanning -60dB to 0dB.
pub fn recording_level() -> Option<f32> {
    let (samples, sample_rate) = live_samples()?;
    let samples = samples.lock().ok()?;
    let window = (sample_rate as usize / 20).min(samples.len());
    if window == 0 {
        return Some(0.0);
    }
    let tail = &samples[samples.len() - window..];
    let rms = (tail.iter().map(|s| s * s).sum::<f32>() / window as f32).sqrt();
    let db = 20.0 * rms.max(1e-6).log10();
    Some(((db + 60.0) / 60.0).clamp(0.0, 1.0))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub enum MicPermission {
    Granted,
    Denied,
    Undetermined,
    /// The app that launched Saytype can't grant microphone access, e.g. an editor's terminal.
    Blocked,
}

/// macOS keeps reporting NotDetermined after a blocked prompt, so remember it.
#[cfg(target_os = "macos")]
static MIC_PROMPT_BLOCKED: AtomicBool = AtomicBool::new(false);

pub fn microphone_permission() -> MicPermission {
    #[cfg(target_os = "macos")]
    {
        use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice};
        let status = audio_media_type()
            .map(|audio| unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) });
        match status {
            Some(AVAuthorizationStatus::Authorized) => MicPermission::Granted,
            Some(AVAuthorizationStatus::NotDetermined) | None => {
                if MIC_PROMPT_BLOCKED.load(Ordering::Relaxed) {
                    MicPermission::Blocked
                } else {
                    MicPermission::Undetermined
                }
            }
            Some(_) => MicPermission::Denied,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        MicPermission::Granted
    }
}

pub async fn request_microphone_permission() -> MicPermission {
    #[cfg(target_os = "macos")]
    {
        let _ = ask_for_microphone().recv().await;
        // A refused prompt records Denied; a launcher that can't prompt records nothing.
        if microphone_permission() == MicPermission::Undetermined {
            MIC_PROMPT_BLOCKED.store(true, Ordering::Relaxed);
        }
        microphone_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        MicPermission::Granted
    }
}

#[cfg(target_os = "macos")]
fn audio_media_type() -> Option<&'static objc2_av_foundation::AVMediaType> {
    unsafe { objc2_av_foundation::AVMediaTypeAudio }
}

/// Returns a receiver instead of awaiting, so the non-Send block is gone before the await.
#[cfg(target_os = "macos")]
fn ask_for_microphone() -> tokio::sync::mpsc::UnboundedReceiver<()> {
    use objc2_av_foundation::AVCaptureDevice;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    if let Some(audio) = audio_media_type() {
        let answered = block2::RcBlock::new(move |_: objc2::runtime::Bool| {
            let _ = tx.send(());
        });
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &answered) };
    }
    rx
}

/// Play a sound when recording starts
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn play_start_sound() {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Tink.aiff")
            .spawn()
            .ok();
    }
    #[cfg(target_os = "windows")]
    {
        play_windows_sound("SystemAsterisk");
    }
}

/// Play a sound when recording stops
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn play_stop_sound() {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Pop.aiff")
            .spawn()
            .ok();
    }
    #[cfg(target_os = "windows")]
    {
        play_windows_sound("SystemExclamation");
    }
}

/// Play a sound when hotkey pressed but app is busy/loading
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn play_busy_sound() {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Funk.aiff")
            .spawn()
            .ok();
    }
    #[cfg(target_os = "windows")]
    {
        play_windows_sound("SystemHand");
    }
}

#[cfg(target_os = "windows")]
fn play_windows_sound(alias: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
    let wide: Vec<u16> = alias.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_ALIAS | SND_ASYNC);
    }
}
