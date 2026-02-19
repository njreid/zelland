use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;

fn model_dir() -> PathBuf {
    let xdg_data = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap();
        format!("{}/.local/share", home)
    });
    PathBuf::from(xdg_data).join("voice-terminal/models/moonshine-tiny")
}

fn fixture_path(name: &str) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn read_wav_samples(path: &std::path::Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("Failed to open WAV file");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "Expected mono WAV");

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
    };

    // Resample to 16kHz if needed
    if spec.sample_rate != 16000 {
        voice_terminal_lib::speech::audio::resample(&samples, spec.sample_rate, 16000)
            .expect("Resampling failed")
    } else {
        samples
    }
}

#[test]
fn test_engine_transcribes_speech() {
    let dir = model_dir();
    if !dir.exists() {
        eprintln!(
            "Skipping E2E test: model not downloaded at {}. Run scripts/download-model.sh",
            dir.display()
        );
        return;
    }

    let wav_path = fixture_path("OSR_uk_000_0028_8k.wav");
    assert!(wav_path.exists(), "Fixture WAV not found: {}", wav_path.display());

    let samples = read_wav_samples(&wav_path);
    assert!(!samples.is_empty(), "WAV file produced no samples");
    assert!(
        samples.len() > 16000, // at least 1 second
        "WAV too short: {} samples",
        samples.len()
    );

    let mut engine = voice_terminal_lib::speech::engine::SpeechEngine::new(&dir)
        .expect("Failed to load speech engine");

    let text = engine.transcribe(&samples).expect("Transcription failed");
    eprintln!("Transcription of speech: {:?}", text);

    // Real speech should produce non-empty output with multiple words
    assert!(!text.trim().is_empty(), "Transcription of speech audio should not be empty");
    let word_count = text.split_whitespace().count();
    assert!(
        word_count >= 5,
        "Expected at least 5 words from 36s of speech, got {}: {:?}",
        word_count,
        text
    );
}

#[test]
fn test_pty_roundtrip_with_text() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to open PTY");

    // Use `cat` as an echo server
    let cmd = CommandBuilder::new("cat");
    let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn cat");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();

    let test_text = "hello world from voice terminal\n";
    writer.write_all(test_text.as_bytes()).unwrap();
    drop(writer); // close stdin so cat exits

    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();
    assert!(
        output.contains("hello world from voice terminal"),
        "PTY output didn't contain expected text: {:?}",
        output
    );
}

#[test]
fn test_full_pipeline_engine_to_pty() {
    let dir = model_dir();
    if !dir.exists() {
        eprintln!("Skipping full pipeline test: model not downloaded");
        return;
    }

    // Step 1: Transcribe speech fixture
    let wav_path = fixture_path("OSR_uk_000_0028_8k.wav");
    if !wav_path.exists() {
        eprintln!("Skipping: fixture WAV not found");
        return;
    }

    let samples = read_wav_samples(&wav_path);
    let mut engine = voice_terminal_lib::speech::engine::SpeechEngine::new(&dir)
        .expect("Failed to load engine");
    let text = engine.transcribe(&samples).expect("Transcription failed");

    // Step 2: Send transcribed text to PTY
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to open PTY");

    let cmd = CommandBuilder::new("cat");
    let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn cat");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();

    let to_send = format!("{}\n", text);
    writer.write_all(to_send.as_bytes()).unwrap();
    drop(writer);

    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    eprintln!("Full pipeline: transcribed={:?}, PTY output={:?}", text, output);
    // Real speech should produce text, verify it made it through the PTY
    assert!(!text.trim().is_empty(), "Transcription should not be empty");
    assert!(
        output.contains(&text),
        "PTY output should contain transcribed text"
    );
}

#[test]
fn test_resampling_roundtrip() {
    // Generate 1s of 440Hz sine at 48kHz
    let from_rate = 48000u32;
    let to_rate = 16000u32;
    let samples: Vec<f32> = (0..from_rate)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / from_rate as f32).sin())
        .collect();

    let resampled = voice_terminal_lib::speech::audio::resample(&samples, from_rate, to_rate)
        .expect("Resampling failed");

    // Should be ~16000 samples
    let expected = to_rate as usize;
    assert!(
        (resampled.len() as i64 - expected as i64).unsigned_abs() < (expected / 5) as u64,
        "Expected ~{} samples, got {}",
        expected,
        resampled.len()
    );

    // Verify signal isn't all zeros
    let max_val = resampled.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    assert!(max_val > 0.1, "Resampled signal is too quiet: max={}", max_val);
}
