use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter};
use rubato::{FftFixedInOut, Resampler};
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
        self.output_path = format!(
            "/tmp/saytype_ptt_{}_{}.wav",
            std::process::id(),
            timestamp
        );

        // Get default input device
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        // Get device's native config
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        self.sample_rate = config.sample_rate().0;

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
                        &config.into(),
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
                        &config.into(),
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
                        &config.into(),
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut buffer) = samples_clone.lock() {
                                // Convert u16 to f32 (centered at 0)
                                for &sample in data {
                                    let normalized =
                                        (sample as f32 - 32768.0) / 32768.0;
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

/// Resample audio from source_rate to target_rate using rubato
fn resample_audio(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, String> {
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    // Calculate chunk size that works well with rubato
    // Use a reasonable chunk size for real-time audio
    let chunk_size = 1024;

    // Create resampler
    let mut resampler = FftFixedInOut::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        chunk_size,
        1, // mono
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let input_frames_per_chunk = resampler.input_frames_next();
    let mut output = Vec::with_capacity(
        (samples.len() as f64 * target_rate as f64 / source_rate as f64) as usize + 1024,
    );

    // Process in chunks
    let mut pos = 0;
    while pos + input_frames_per_chunk <= samples.len() {
        let input_chunk = vec![samples[pos..pos + input_frames_per_chunk].to_vec()];
        let resampled = resampler
            .process(&input_chunk, None)
            .map_err(|e| format!("Resampling error: {}", e))?;
        output.extend_from_slice(&resampled[0]);
        pos += input_frames_per_chunk;
    }

    // Handle remaining samples by padding with zeros
    if pos < samples.len() {
        let remaining = samples.len() - pos;
        let mut padded = samples[pos..].to_vec();
        padded.resize(input_frames_per_chunk, 0.0);
        let input_chunk = vec![padded];
        let resampled = resampler
            .process(&input_chunk, None)
            .map_err(|e| format!("Resampling error: {}", e))?;
        // Only take the proportion of output that corresponds to actual input
        let output_frames =
            (remaining as f64 * target_rate as f64 / source_rate as f64) as usize;
        output.extend_from_slice(&resampled[0][..output_frames.min(resampled[0].len())]);
    }

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

/// Check if microphone permission is granted using AVFoundation API
pub fn check_microphone_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Try AVFoundation API first
        let status = get_av_authorization_status();
        eprintln!("check_microphone_permission: AVAuthorizationStatus = {}", status);

        // If AVFoundation works and returns authorized, we're good
        if status == 3 {
            return true;
        }

        // If status is 1 (restricted) or 2 (denied), we know permission is not granted
        if status == 1 || status == 2 {
            return false;
        }

        // If status is 0 (not determined) or AVFoundation failed,
        // fall back to trying to actually use the microphone
        check_microphone_by_recording()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Fallback: check microphone permission by trying to record
#[cfg(target_os = "macos")]
fn check_microphone_by_recording() -> bool {
    eprintln!("check_microphone_by_recording: trying to access microphone");

    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("check_microphone_by_recording: no input device");
            return false;
        }
    };

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("check_microphone_by_recording: config error: {}", e);
            return false;
        }
    };

    // Try to build and start a stream
    let stream = device.build_input_stream_raw(
        &config.config(),
        config.sample_format(),
        move |_data: &cpal::Data, _: &cpal::InputCallbackInfo| {},
        move |_err| {},
        None,
    );

    match stream {
        Ok(s) => {
            match s.play() {
                Ok(_) => {
                    // Successfully started recording - permission granted
                    eprintln!("check_microphone_by_recording: stream started successfully");
                    drop(s);
                    true
                }
                Err(e) => {
                    eprintln!("check_microphone_by_recording: play error: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            eprintln!("check_microphone_by_recording: stream error: {}", e);
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn get_av_authorization_status() -> i64 {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use std::ffi::CStr;

    // Link AVFoundation framework to ensure AVCaptureDevice class is available
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    unsafe {
        // Get AVCaptureDevice class
        let av_capture_device_class = match AnyClass::get(c"AVCaptureDevice") {
            Some(cls) => cls,
            None => {
                eprintln!("AVCaptureDevice class not found");
                return 0;
            }
        };

        // Get AVMediaTypeAudio - the actual constant value is "soun"
        let ns_string_class = match AnyClass::get(c"NSString") {
            Some(cls) => cls,
            None => {
                eprintln!("NSString class not found");
                return 0;
            }
        };

        let audio_type_cstr = CStr::from_bytes_with_nul(b"soun\0").unwrap();
        let audio_type: *mut AnyObject =
            msg_send![ns_string_class, stringWithUTF8String: audio_type_cstr.as_ptr()];

        // Call [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]
        let status: i64 = msg_send![av_capture_device_class, authorizationStatusForMediaType: audio_type];

        status
    }
}

/// Play a sound when recording starts
pub fn play_start_sound() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Sounds/Tink.aiff")
        .spawn()
        .ok();
}

/// Play a sound when recording stops
pub fn play_stop_sound() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Sounds/Pop.aiff")
        .spawn()
        .ok();
}

/// Play a sound when hotkey pressed but app is busy/loading
pub fn play_busy_sound() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Sounds/Funk.aiff")
        .spawn()
        .ok();
}

/// Request microphone permission and wait for user response
pub async fn request_microphone_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // Check current authorization status first
        let status = get_av_authorization_status();
        eprintln!("request_microphone_permission: initial status = {}", status);

        match status {
            3 => return Ok(true),  // Already authorized
            2 => return Ok(false), // Denied - user must change in System Preferences
            1 => return Ok(false), // Restricted
            _ => {
                // 0 = Not determined, or unknown - need to request permission
            }
        }

        // Trigger the permission request by attempting to use the microphone
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => return Err("No input device available".to_string()),
        };

        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to get input config: {}", e)),
        };

        // Build an input stream - this triggers the permission dialog
        let stream = device.build_input_stream_raw(
            &config.config(),
            config.sample_format(),
            move |_data: &cpal::Data, _: &cpal::InputCallbackInfo| {
                // Do nothing
            },
            move |_err| {
                // Ignore errors
            },
            None,
        );

        match stream {
            Ok(s) => {
                // Try to play the stream to trigger permission
                if s.play().is_ok() {
                    eprintln!("request_microphone_permission: stream started, waiting for user response");

                    // Wait for the dialog to appear and user to respond
                    // Poll for up to 30 seconds, checking both AVFoundation status and actual recording capability
                    for i in 0..300 {
                        std::thread::sleep(std::time::Duration::from_millis(100));

                        // Check AVFoundation status
                        let current_status = get_av_authorization_status();
                        if i % 20 == 0 {
                            eprintln!("request_microphone_permission: polling iteration {}, status = {}", i, current_status);
                        }

                        // If status changed from "not determined", we have an answer
                        if current_status == 3 {
                            drop(s);
                            eprintln!("request_microphone_permission: authorized via AVFoundation");
                            return Ok(true);
                        } else if current_status == 2 {
                            drop(s);
                            eprintln!("request_microphone_permission: denied via AVFoundation");
                            return Ok(false);
                        }

                        // Every 2 seconds, also try the fallback check
                        if i > 0 && i % 20 == 0 && current_status == 0 {
                            // AVFoundation might not be working, try actual recording test
                            if check_microphone_by_recording() {
                                drop(s);
                                eprintln!("request_microphone_permission: authorized via recording test");
                                return Ok(true);
                            }
                        }
                    }
                }
                drop(s);

                // Timeout - do final checks
                let final_status = get_av_authorization_status();
                eprintln!("request_microphone_permission: timeout, final AVFoundation status = {}", final_status);

                if final_status == 3 {
                    return Ok(true);
                } else if final_status == 2 {
                    return Ok(false);
                }

                // Final fallback: try recording
                let can_record = check_microphone_by_recording();
                eprintln!("request_microphone_permission: fallback recording test = {}", can_record);
                Ok(can_record)
            }
            Err(e) => {
                // Stream creation failed - might be permission denied
                let current_status = get_av_authorization_status();
                eprintln!("request_microphone_permission: stream failed, status = {}, error = {}", current_status, e);

                if current_status == 3 {
                    Ok(true)
                } else if current_status == 2 {
                    Ok(false)
                } else {
                    // Try fallback
                    let can_record = check_microphone_by_recording();
                    if can_record {
                        Ok(true)
                    } else {
                        Err(format!("Failed to create audio stream: {}", e))
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let host = cpal::default_host();
        match host.default_input_device() {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
}
