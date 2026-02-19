# Speech Transcription Design

Push-to-talk speech-to-text, currently implemented as a standalone voice terminal prototype in `src-voice/`.

## Implementation Status

A working prototype exists at `src-voice/` — a standalone Tauri v2 + SvelteKit 5 app that pairs PTT speech transcription with a full xterm.js terminal. The transcribed text is previewed in an editable bar, then sent as shell input on Enter.

**What works:**

- Moonshine Tiny inference via split ONNX encoder/decoder with KV cache
- BPE tokenization via `tokenizers` crate with `tokenizer.json`
- cpal audio capture with mono conversion and rubato resampling
- PTY shell spawning, I/O, and resize via `portable-pty`
- Alt+Space toggle recording, preview bar for editing before send
- E2E tests: real WAV transcription, PTY roundtrip, full pipeline

**Not yet done:**

- Vulkan EP acceleration (CPU-only inference)
- Model warm-up on init (first inference is slow)
- Audio settings (input device, input volume)

## Constraints

- **Tauri v2** — no sidecar support on mobile, so the entire stack must be embedded in the Rust backend
- **Push-to-talk** — user holds a key (Alt+Space), releases to transcribe; no streaming/VAD needed
- **Platforms**: Linux desktop (WebKit), Android (WebView)

## Model: Moonshine Tiny

Whisper uses a fixed 30-second encoder window — a 2-second PTT clip wastes compute on 28 seconds of silence padding. Moonshine uses a variable-length encoder, doing ~1/15th the work of Whisper on short clips.

| | Moonshine Tiny | Whisper Tiny |
|---|---|---|
| Params | 27M | 39M |
| ONNX size | ~27 MB | ~75 MB |
| Short-clip latency | 5-15x faster | Baseline |
| Input | Variable-length 16kHz f32 | Fixed 30s window |

### Model Architecture (as implemented)

Moonshine uses a two-pass architecture:

1. **Encoder**: Conv2D audio encoder, input `[1, num_samples]` f32 → output `[1, seq_len, hidden_size]` embeddings
2. **Decoder**: Autoregressive transformer — 6 layers, 8 attention heads, 36-dim head size
   - KV cache: 24 tensors (6 layers × 4: decoder key/val + cross-attention key/val)
   - First step: full token sequence, `use_cache_branch=false`
   - Subsequent steps: last token only, `use_cache_branch=true`
   - Greedy decoding (argmax over logits)
   - Special tokens: SOT=1 (start), EOT=2 (end)
   - Max 500 decoding steps

### Model Files

Three files downloaded from `huggingface.co/onnx-community/moonshine-tiny-ONNX`:

```text
$XDG_DATA_HOME/voice-terminal/models/moonshine-tiny/
  encoder_model.onnx
  decoder_model_merged.onnx
  tokenizer.json
```

Download script: `src-voice/scripts/download-model.sh`

**Note**: The original design assumed a single bundled `moonshine-tiny.onnx` via Tauri resources. The implementation uses split encoder/decoder models with a separate BPE tokenizer, downloaded to user data dir rather than bundled. For zelland integration, consider bundling via Tauri resources or downloading on first use.

## Inference Stack

| Component | Choice | Version | Notes |
|---|---|---|---|
| Model format | ONNX | — | Split encoder + decoder |
| Inference | `ort` crate | 2.0.0-rc.11 | `load-dynamic` feature |
| Tokenizer | `tokenizers` crate | 0.21 | BPE, `tokenizer.json` from HuggingFace |
| Tensor ops | `ndarray` | 0.17 | Reshape/extract for ONNX I/O |
| Audio capture | `cpal` | 0.15 | Default input device, f32 stream |
| Resampling | `rubato` | 0.14 | Sinc interpolation, 48kHz→16kHz |
| PTY | `portable-pty` | 0.9 | Shell spawning (voice terminal only) |

## Rust Backend (src-voice/src-tauri/src/)

### Module Layout

```text
src-tauri/src/
  lib.rs            # App setup, state management, command registration
  main.rs           # Entry point (calls lib::run())
  pty.rs            # PTY spawning, I/O, resize
  speech/
    mod.rs          # SpeechState + Tauri command handlers
    engine.rs       # SpeechEngine — encoder/decoder sessions, inference loop
    audio.rs        # AudioRecorder::run_blocking() + resample()
```

### SpeechEngine (engine.rs)

Holds persistent encoder and decoder `ort::Session` instances plus a `tokenizers::Tokenizer`.

```rust
pub struct SpeechEngine {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
}

impl SpeechEngine {
    pub fn new(model_dir: &Path) -> anyhow::Result<Self>;
    pub fn transcribe(&mut self, samples: &[f32]) -> anyhow::Result<String>;
}
```

- `new()`: verifies all 3 model files exist, loads encoder/decoder with 4 intra-op threads each, loads tokenizer from JSON
- `transcribe()`: rejects < 1600 samples (~100ms), runs encoder pass, initializes KV cache (24 zero tensors), runs autoregressive decoder loop with greedy argmax, decodes token IDs via tokenizer
- `unsafe impl Send + Sync` — ort::Session is thread-safe in practice

### AudioRecorder (audio.rs)

Zero-sized struct with static methods. Recording runs on a dedicated OS thread because `cpal::Stream` is `!Send`.

```rust
pub struct AudioRecorder;

impl AudioRecorder {
    pub fn run_blocking(
        buffer: Arc<Mutex<Vec<f32>>>,
        device_rate: Arc<Mutex<u32>>,
        stop_signal: Arc<Mutex<bool>>,
    ) -> anyhow::Result<()>;
}

pub fn resample(samples: Vec<f32>, from_rate: u32, to_rate: u32) -> anyhow::Result<Vec<f32>>;
```

- `run_blocking()`: opens default input device, converts multi-channel to mono (average), streams f32 samples into mutex buffer, polls stop signal every 10ms
- `resample()`: sinc interpolation via rubato (256-length, 0.95 cutoff, Blackman-Harris2 window, 256× oversampling)

### SpeechState (speech/mod.rs)

```rust
pub struct SpeechState {
    pub engine: Option<SpeechEngine>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    device_sample_rate: Arc<Mutex<u32>>,
    stop_signal: Arc<Mutex<bool>>,
    recording: bool,
    record_thread: Option<JoinHandle<()>>,
}
```

Managed as `Arc<Mutex<SpeechState>>` in Tauri state.

### Tauri Commands

```rust
#[tauri::command]
async fn init_speech(app: AppHandle, state: State<'_, Arc<Mutex<SpeechState>>>) -> Result<(), String>;

#[tauri::command]
async fn start_recording(state: State<'_, Arc<Mutex<SpeechState>>>) -> Result<(), String>;

#[tauri::command]
async fn stop_and_transcribe(state: State<'_, Arc<Mutex<SpeechState>>>) -> Result<String, String>;
```

- `init_speech`: spawns blocking task to load model from `$XDG_DATA_HOME/voice-terminal/models/moonshine-tiny/`, emits `speech-ready` event on success
- `start_recording`: clears buffer, resets stop signal, spawns OS thread with `AudioRecorder::run_blocking()`
- `stop_and_transcribe`: sets stop signal, joins thread, resamples if needed, runs `engine.transcribe()` in blocking task

### PTY Module (pty.rs)

```rust
pub struct PtyState {
    tx: mpsc::Sender<Vec<u8>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
}
```

- `pty_spawn`: opens PTY pair (24×80), spawns `$SHELL` (fallback `/bin/sh`), reader thread emits `pty-output` events (8KB chunks), writer task reads from mpsc channel
- `pty_write`: sends byte data to PTY via mpsc channel
- `pty_resize`: resizes PTY master to given rows/cols

### Tauri Events

| Event | Direction | Payload |
|---|---|---|
| `speech-ready` | Backend → Frontend | none |
| `speech-error` | Backend → Frontend | error string |
| `pty-output` | Backend → Frontend | terminal output string |
| `pty-closed` | Backend → Frontend | none |

## Frontend (src-voice/src/)

Standalone SvelteKit 5 app with SSR disabled (SPA mode). Tokyonight dark color scheme.

### Components

**`+page.svelte`** — main page (~254 lines)

- Listens for `speech-ready`, `speech-error`, `pty-closed` events on mount
- Calls `init_speech` on startup (non-blocking — app works without speech)
- Alt+Space toggles recording (calls `start_recording` / `stop_and_transcribe`)
- Shows preview bar with editable transcription; Enter sends to PTY, Esc cancels
- Ctrl+Shift+R restarts shell when PTY exits
- Status bar shows recording state (pulsing red dot) and hint text

**`VoiceTerminal.svelte`** — xterm.js wrapper (~108 lines)

- Creates Terminal with Tokyonight theme, Inconsolata font, 5000-line scrollback
- FitAddon for responsive sizing, ResizeObserver triggers `pty_resize`
- `term.onData()` → `pty_write` (user typing to PTY)
- Listens `pty-output` → `term.write()` (PTY output to screen)
- Exports `sendToPty(text)` for voice input injection

### Dependencies

- `@tauri-apps/api@2` — IPC bridge
- `@xterm/xterm@6.0.0` — terminal emulator
- `@xterm/addon-fit@0.11.0` — auto-sizing

## Data Flow

```text
User presses Alt+Space
  → invoke('start_recording')
  → OS thread: cpal opens mic, streams f32 mono samples into Arc<Mutex<Vec<f32>>>

User presses Alt+Space again
  → invoke('stop_and_transcribe')
  → stop_signal = true, join recording thread
  → drain audio buffer
  → rubato resamples device rate → 16kHz (if needed)
  → encoder pass: [1, N] f32 → [1, seq_len, hidden] embeddings
  → decoder loop: autoregressive with KV cache, greedy argmax
  → tokenizer.decode(token_ids) → String
  → returned to frontend

Frontend shows preview bar
  → user edits transcription
  → Enter: sendToPty(text + '\n') → pty_write → shell stdin
  → Esc: discard

Shell output
  → reader thread: 8KB chunks → emit 'pty-output'
  → VoiceTerminal: term.write(data)
```

## Tests

### Unit Tests (engine.rs, audio.rs)

- `test_engine_loads` — load model (skip if missing)
- `test_transcribe_too_short` — reject < 1600 samples
- `test_transcribe_dummy_audio` — 1s silence doesn't crash
- `test_resample_48k_to_16k` — 440Hz sine, verify output length
- `test_resample_identity` — same rate returns same samples

### Integration Tests (tests/e2e_speech.rs)

- `test_engine_transcribes_speech` — transcribe 36s UK speech WAV, assert ≥5 words
- `test_pty_roundtrip_with_text` — spawn cat, echo test
- `test_full_pipeline_engine_to_pty` — transcribe → send to PTY → verify output
- `test_resampling_roundtrip` — 48kHz→16kHz, verify sample count and amplitude

**Fixture**: `tests/fixtures/OSR_uk_000_0028_8k.wav` (36s UK English @ 8kHz)

## Integration Plan: src-voice → zelland

The prototype lives in `src-voice/` as a standalone app. To bring speech into zelland:

1. **Extract reusable modules**: `speech/engine.rs`, `speech/audio.rs`, and `speech/mod.rs` can move into `src-tauri/src/speech/` with minimal changes (replace `voice-terminal` model path with zelland's)
2. **VoiceTextarea component**: The original design target — a textarea wrapper with PTT button for annotation input. The speech backend is ready; the frontend component still needs to be built for the zelland UI (PicoCSS + Lucide icons, not xterm.js)
3. **Model distribution**: Decide between bundling via Tauri resources or downloading on first use (current approach). For Android, bundling avoids network dependency
4. **PTY module**: Not needed in zelland (SSH terminal is a separate feature)

## Resolved Questions

These were open in the original design and are now answered by the implementation:

- **Tokenizer**: ONNX model outputs raw token IDs. A separate `tokenizer.json` (BPE) is required, loaded via the `tokenizers` crate
- **transcribe-rs**: Not used. Custom encoder/decoder implementation with KV cache management
- **Model architecture**: Two-pass (encoder + autoregressive decoder), not single-session. Requires split ONNX files

## Remaining Open Questions

- **Vulkan EP**: Not yet implemented. Need graceful fallback to CPU if Vulkan init fails on Linux
- **Model warm-up**: First inference is slow. Consider running a dummy inference on init
- **Android audio permissions**: How/when to prompt for RECORD_AUDIO (on first PTT press vs app startup)
- **ort version**: Using 2.0.0-rc.11 (release candidate). Pin to stable release when available
- **Model quantization**: Current models are float32. Int8 quantization would reduce size and latency
