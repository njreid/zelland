pub mod audio;
pub mod engine;

use self::audio::AudioRecorder;
use self::engine::SpeechEngine;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

/// Tauri-managed state: only holds Send+Sync types.
/// The AudioRecorder (which contains cpal::Stream, !Send) is managed
/// on a dedicated thread and accessed via channels.
pub struct SpeechState {
    pub engine: Option<SpeechEngine>,
    /// Shared audio buffer — recorder thread writes, main thread reads
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    /// Device sample rate captured at recording start
    device_sample_rate: Arc<Mutex<u32>>,
    /// Signal to stop recording
    stop_signal: Arc<Mutex<bool>>,
    /// Whether recording is active
    recording: bool,
    /// Handle to the recording thread
    record_thread: Option<std::thread::JoinHandle<()>>,
    /// Selected audio input device name (None = default)
    selected_device: Option<String>,
    /// Input gain multiplier (0.0–2.0, default 1.0)
    input_gain: f32,
}

impl SpeechState {
    pub fn new() -> Self {
        Self {
            engine: None,
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            device_sample_rate: Arc::new(Mutex::new(48000)),
            stop_signal: Arc::new(Mutex::new(false)),
            recording: false,
            record_thread: None,
            selected_device: None,
            input_gain: 1.0,
        }
    }
}

fn model_dir() -> PathBuf {
    let xdg_data = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap();
        format!("{}/.local/share", home)
    });
    PathBuf::from(xdg_data).join("voice-terminal/models/moonshine-tiny")
}

#[tauri::command]
pub async fn init_speech(
    app_handle: AppHandle,
    speech_state: tauri::State<'_, Arc<Mutex<SpeechState>>>,
) -> Result<(), String> {
    let dir = model_dir();
    info!("Loading speech engine from {}", dir.display());

    let engine = tokio::task::spawn_blocking(move || {
        let mut engine = SpeechEngine::new(&dir)?;
        engine.warm_up();
        Ok::<_, anyhow::Error>(engine)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| {
        error!("Failed to load speech engine: {}", e);
        e.to_string()
    })?;

    {
        let mut state = speech_state.lock().map_err(|e| e.to_string())?;
        state.engine = Some(engine);
    }

    let _ = app_handle.emit("speech-ready", ());
    info!("Speech engine ready");
    Ok(())
}

#[tauri::command]
pub async fn start_recording(
    speech_state: tauri::State<'_, Arc<Mutex<SpeechState>>>,
) -> Result<(), String> {
    let mut state = speech_state.lock().map_err(|e| e.to_string())?;
    if state.recording {
        return Err("Already recording".to_string());
    }

    // Clear buffer and reset stop signal
    {
        let mut buf = state.audio_buffer.lock().unwrap();
        buf.clear();
    }
    {
        let mut stop = state.stop_signal.lock().unwrap();
        *stop = false;
    }

    let buffer = state.audio_buffer.clone();
    let device_rate = state.device_sample_rate.clone();
    let stop_signal = state.stop_signal.clone();
    let device_name = state.selected_device.clone();
    let gain = state.input_gain;

    // Spawn recording on a dedicated OS thread (cpal::Stream is !Send)
    let handle = std::thread::spawn(move || {
        match AudioRecorder::run_blocking(buffer, device_rate, stop_signal, device_name, gain) {
            Ok(()) => info!("Recording thread finished"),
            Err(e) => error!("Recording thread error: {}", e),
        }
    });

    state.recording = true;
    state.record_thread = Some(handle);
    Ok(())
}

#[tauri::command]
pub async fn stop_and_transcribe(
    speech_state: tauri::State<'_, Arc<Mutex<SpeechState>>>,
) -> Result<String, String> {
    // Clone the Arc so we can use it across await points
    let state_arc = speech_state.inner().clone();

    // Signal stop and wait for recording thread
    let (samples, device_rate) = {
        let mut state = state_arc.lock().map_err(|e| e.to_string())?;
        if !state.recording {
            return Err("Not recording".to_string());
        }

        // Signal stop
        {
            let mut stop = state.stop_signal.lock().unwrap();
            *stop = true;
        }

        // Take the thread handle
        let handle = state.record_thread.take();
        drop(state); // release lock while waiting for thread

        if let Some(handle) = handle {
            handle.join().map_err(|_| "Recording thread panicked".to_string())?;
        }

        let mut state = state_arc.lock().map_err(|e| e.to_string())?;
        state.recording = false;

        let samples = {
            let mut buf = state.audio_buffer.lock().unwrap();
            std::mem::take(&mut *buf)
        };
        let rate = *state.device_sample_rate.lock().unwrap();
        (samples, rate)
    };

    if samples.is_empty() {
        return Ok(String::new());
    }

    // Resample if needed
    let samples_16k = if device_rate != 16000 {
        audio::resample(&samples, device_rate, 16000).map_err(|e| e.to_string())?
    } else {
        samples
    };

    info!("Transcribing {} samples (16kHz)", samples_16k.len());

    // Run transcription on blocking thread
    let arc_for_blocking = state_arc.clone();
    let text = tokio::task::spawn_blocking(move || {
        let mut state = arc_for_blocking.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut engine) = state.engine {
            engine.transcribe(&samples_16k).map_err(|e| e.to_string())
        } else {
            Err("Speech engine not loaded".to_string())
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    Ok(text)
}

#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<audio::AudioDeviceInfo>, String> {
    audio::list_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_audio_device(
    speech_state: tauri::State<'_, Arc<Mutex<SpeechState>>>,
    device_name: Option<String>,
) -> Result<(), String> {
    let mut state = speech_state.lock().map_err(|e| e.to_string())?;
    info!("Audio device set to: {:?}", device_name);
    state.selected_device = device_name;
    Ok(())
}

#[tauri::command]
pub fn set_input_gain(
    speech_state: tauri::State<'_, Arc<Mutex<SpeechState>>>,
    gain: f32,
) -> Result<(), String> {
    let clamped = gain.clamp(0.0, 2.0);
    let mut state = speech_state.lock().map_err(|e| e.to_string())?;
    info!("Input gain set to: {:.2}", clamped);
    state.input_gain = clamped;
    Ok(())
}
