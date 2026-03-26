# zelland Implementation Plan

This document tracks the evolution of zelland into a resilient mobile command center using WireGuard tunneling

## Overview

- **Target Platforms:** Android, Linux Desktop
- **Tech Stack:** Tauri v2, Svelte 5 (Runes), Rust (GotaTun, Russh), Ghostty VT, wgpu
- **Architecture:** Userspace WireGuard Tunnel -> SSH over UDP -> Local Svelte 5 UI

## Completed Foundation

- [x] Tauri v2 Project Scaffolding
- [x] Basic Svelte 5 + Tailwind Setup
- [x] Initial SSH/Terminal Components (to be refactored)
- [x] Basic Android Intent Handling

## Milestones

### Phase 9: WGPU + Ghostty Remediation

- Status: partially complete. The Ghostty + wgpu migration landed, but the repo does not support the old plan's "COMPLETED" claim yet.

- **Technical direction preserved from `WGPU_GHOSTTY_PLAN.md`:** `libghostty-vt` (Zig) static library backend, `wgpu` + `glyphon` native Android rendering, JNI for `Surface`/touch handoff, zero-scrollback local terminal state.

#### 9A: Phase 0 - Testing Harness (Dinghy)

- [x] Configure `dinghy.toml` for Android test execution.
- [x] Validate Android test infrastructure with `test_android_connectivity`.
- [x] Add Ghostty-focused test coverage in `src-tauri/tests/ghostty_vt_test.rs` for the FFI wrapper and render state.

#### 9B: Phase 1 - Hybrid Mode (`libghostty-vt` backend + `xterm.js` frontend)

- [x] Integrate Zig toolchain build steps in `src-tauri/build.rs`.
- [x] Generate Rust FFI bindings with `bindgen` in `src-tauri/build.rs`.
- [x] Refactor `TerminalSession` to use `GhosttyTerminalWrapper`.
- [x] Validate the Ghostty VT engine through the interim hybrid path before full native rendering.

#### 9C: Phase 2 - Native Surface and wgpu Foundation

- [x] Add Android JNI surface hooks for native renderer startup in `src-tauri/src/renderer/android.rs`.
- [x] Initialize the native renderer stack with `wgpu`.
- [x] Integrate `glyphon` for monospace text rendering.
- [x] Prove the native surface path with initial terminal-grid rendering.
- [ ] Check in and document the Android `MainActivity.kt` / `SurfaceView` implementation so the lifecycle path is reviewable from the repo.

#### 9D: Phase 3 - Full Integration (Ghostty render state -> native surface)

- [x] Implement `render_native` in `TerminalSession` and drive it from `SshManager`.
- [x] Implement Ghostty render-state iteration and row dirty inspection in `GhosttyRenderStateWrapper` and `Renderer`.
- [x] Add JNI touch forwarding through `passTouchToRust` and Ghostty mouse-event encoding.
- [x] Set `max_scrollback` to `0` for the native terminal path.
- [x] Make the terminal WebView container transparent so the native surface is visible.
- [ ] Fix dropped-frame behavior when Ghostty dirty flags are cleared before a renderer/surface is attached.
- [ ] Replace partial row caching with end-to-end incremental rendering so unchanged rows do not trigger full viewport shaping.
- [ ] Complete terminal styling support: background colors, underline, reverse video, cursor rendering, and non-debug clear/background behavior.
- [ ] Replace duplicated `24x32` cell-size constants with measured renderer metrics shared by resize and mouse hit-testing.
- [ ] Send real pixel dimensions through SSH PTY resize instead of `0, 0`.
- [ ] Remove the global singleton renderer design in favor of explicit surface/renderer ownership per session or view.

#### 9E: Cleanup, Validation, and Follow-up

- [x] Create `docs/features/WGPU_GHOSTTY_REMEDIATION_DESIGN.md` to document the remediation pass.
- [x] Move Android-specific renderer dependencies behind Android-only Cargo target sections.
- [x] Fix `glyphon` renderer setup so host builds compile again.
- [x] Remove the redundant ANSI viewport payload from the native render flush path.
- [x] Remove stale main-app `xterm` dependencies.
- [x] Add focused Rust tests for Ghostty mouse encoding.
- [x] Update `TESTING.md` for the native terminal stack.
- [x] Audit the WGPU + Ghostty implementation against `WGPU_GHOSTTY_PLAN.md` and capture refactoring notes in `CODEX_NOTES.md`.
- [ ] Add a renderer regression test for surface-not-ready / surface-recreated redraw behavior.
- [ ] Add a resize/orientation lifecycle test for Android surface recreation.
- [ ] Add renderer-focused verification that unchanged rows are not fully rebuilt after incremental terminal updates.

### Phase 6: Daemon Migration (Go → Rust) & Annotation System

[x] **Daemon Migration (Go → Rust)**: The companion daemon (`zlnd`) has been rewritten from Go to Rust, enabling first-class YJS CRDT support. The Rust daemon (`daemon-rs`) matches the legacy Go API while adding robust real-time synchronization. See `DAEMON_DESIGN.md` for architecture and `ANNOTATION_DESIGN.md` for the annotation system design.

#### 6A: Rust Daemon Scaffold

- [x] Create `daemon-rs/` Cargo project with `axum`, `tokio`, `kdl`, `notify`, `prost`, `clap` dependencies.
- [x] **Config module** (`config.rs`): KDL config parsing (`port`, `cert_file`, `key_file`, `projects_path`). Default to port 8083, `~/code`.
- [x] **Tests:** Config load, default values, merge with CLI flags.

#### 6B: Core Data & Persistence

- [x] **Project types** (`projects.rs`): `Project` struct, directory scanning (replaces Go `kdl/projects.go`).
- [x] **Annotation types** (`store.rs`): `Annotation` struct with KDL serde. Load/Save/Append (upsert) for `.ann.kdl` files.
- [x] **Tests:** KDL roundtrip for projects and annotations, upsert logic, missing file handling.

#### 6C: Asset Manager

- [x] **Asset manager** (`assets.rs`): Random ID generation, TTL-based expiry (30 min), cleanup task, file serving.
- [x] **Tests:** Register, serve, expiry, cleanup.

#### 6D: HTTP Server (axum)

- [x] **REST endpoints** matching Go API:
  - `GET /api/v1/projects` — list projects from directory scan.
  - `POST /api/v1/projects/activate` — acknowledge activation.
  - `GET /api/v1/fs/read?path=` — read file content (with path security check).
  - `GET /assets/{id}` — serve registered assets.
  - `POST /api/v1/trigger/show` — register + broadcast image (loopback only).
  - `POST /api/v1/trigger/md` — register + broadcast markdown (loopback only).
- [x] **Loopback guard** middleware for trigger endpoints.
- [x] **Tests:** Each endpoint with mock requests, loopback enforcement.

#### 6E: WebSocket & Protobuf

- [x] **WebSocket handler** (`/ws`): Binary protobuf over WebSocket (same `zelland.proto`).
- [x] **Client registry**: Track connected clients, broadcast to all.
- [x] **Message dispatch**: Handle `AnnotationAction`, forward to store.
- [x] **KeepAlive** ping on connect.
- [x] **Tests:** WebSocket connect/disconnect, protobuf round-trip, broadcast fan-out.

#### 6F: File Watching

- [x] **File watcher** (`notify` crate): Watch registered assets, broadcast `OpenViewRequest` on change.
- [x] **Tests:** File type detection (md, images, pdf, unknown).

#### 6G: CLI Binaries

- [x] **`zlnd`** (`main.rs`): Entry point with `clap` flags (`--config`, `--port`).
- [x] **`zn`** (separate `[[bin]]`): CLI tool for `show` and `md` trigger commands via localhost HTTP.
- [x] **Integration tests:** Full API flow, WebSocket connect + ping, trigger from loopback.

### 6I: Integration & Cutover

- [x] Verify Tauri client (`daemon.rs`) works against new Rust daemon (same proto, same REST API).
- [x] Update `Taskfile.yml` `dev:daemon` to build/run Rust daemon.
- [x] Remove Go `daemon/` directory.
- [x] Update `DAEMON_DESIGN.md` and `ANNOTATION_DESIGN.md`.

### Phase 7: Annotation Client (Svelte)

[x] **Annotation Client (Svelte)**: Built the Svelte-side annotation system with YJS CRDT sync, inline rendering, and a sidecar mutation system.

#### 7A: Core Plumbing

- [x] Install `yjs`, `y-protocols`, `lib0` npm dependencies.
- [x] Create `src/lib/annotations.ts` — annotation manager with Y.Doc + custom WebSocket provider (y-websocket binary protocol), reactive `Ann[]` via Svelte 5 runes.
- [x] Create `src/lib/marked-annotations.ts` — custom `marked` inline extension for `[|ID|]` anchors + post-render highlight function.
- [x] Integrate into `MarkdownPane.svelte` — connect on session, disconnect on destroy, highlight after render.

#### 7B: Annotation Sidebar UI

- [x] Create `AnnotationSidebar.svelte` — list of annotations per document, click to scroll.
- [x] Create `AnnotationForm.svelte` — text selection → new annotation + first comment.
- [x] Wire sidebar into page layout (collapsible panel).

#### 7C: Annotation CRUD

- [x] Create annotation from text selection (quote + prefix/suffix context).
- [x] Add comment to existing annotation thread.
- [x] Delete annotation.
- [x] Optimistic local updates via YJS (no REST needed).
- [x] **Source Mutation**: Added daemon endpoint to insert `[|ID|]` markers into Markdown source.

#### 7D: Polish & Edge Cases

- [x] Handle reconnect on network failure (exponential backoff).
- [x] Handle document switch (disconnect old, connect new).
- [x] Fuzzy text matching fallback when marker anchors are absent.
- [x] Mobile gesture support for text selection (`selectionchange` tracking).

### Phase 8: Voice Transcription (PTT)

Integrate push-to-talk speech-to-text from the `src-voice/` prototype into the main app, using embedded ONNX models for Desktop and system ASR APIs for Android.

#### 8A: Core Speech Backend (Rust)

- [ ] **Port Speech Modules**: Move `speech/mod.rs`, `speech/engine.rs`, and `speech/audio.rs` from `src-voice` to `src-tauri/src/speech/`.
- [ ] **Dependency Management**: Add `ort` (v2.0.0-rc.11), `tokenizers`, `cpal`, `rubato`, `ndarray`, and `hound` (dev-dependency) to `src-tauri/Cargo.toml`.
- [ ] **Platform Abstraction**: Refactor `SpeechState` to support different engines per platform.
  - [ ] Linux/Desktop: Use `SpeechEngine` with Moonshine Tiny (ONNX).
  - [ ] Android: Use system-provided ASR.
- [ ] **Tauri Commands**: Register `init_speech`, `start_recording`, `stop_and_transcribe`, `set_audio_device`, and `set_input_gain`.

#### 8B: Android System ASR

- [ ] **Permissions**: Add `android.permission.RECORD_AUDIO` to `AndroidManifest.xml`.
- [ ] **Kotlin Bridge**: Implement `SpeechRecognizer` in `MainActivity.kt`.
  - [ ] Handle `RecognitionListener` events (partial results, final result, errors).
  - [ ] Create JNI methods or Tauri events to trigger/stop transcription.
- [ ] **Rust Integration**: Implement `AndroidSpeechEngine` that communicates with the Kotlin side via `tauri::Emitter`.

#### 8C: Frontend Integration (Svelte 5)

- [ ] **VoiceInput Component**: Build a reusable PTT button + preview bar component with Svelte 5 runes.
- [ ] **Annotation Integration**: Add voice input to `AnnotationForm.svelte` for hands-free commenting.
- [ ] **Terminal Integration**: Add voice input shortcut (Alt+Space) to `Terminal.svelte` to inject text into the shell.

#### 8D: Verification & Ported Tests

- [ ] **Port Unit Tests**: Ensure `engine.rs` and `audio.rs` tests are running in the new context.
- [ ] **Port Integration Tests**: Move `e2e_speech.rs` to `src-tauri/tests/` and verify with the `hound` fixture.
- [ ] **Regression Testing**: Verify existing SSH, WireGuard, and Annotation features are unaffected by the new audio/inference dependencies.

### Future: Zellij Action Buttons

Expose `zellij -s $SESSION action <action>` via `runZellijAction()` as UI buttons (TopBar for desktop, KeybarNative for mobile).

- [ ] `new-tab` / `close-tab` — tab lifecycle
- [ ] `go-to-next-tab` / `go-to-previous-tab` — cycle tabs (better than numbered for >3 tabs)
- [ ] `toggle-fullscreen` — maximize/restore focused pane (very useful on small screens)
- [ ] `toggle-floating-panes` — show/hide floating panes
- [ ] `new-pane` / `close-pane` — pane lifecycle
- [ ] `focus-next-pane` / `focus-previous-pane` — navigate between panes
- [ ] `toggle-pane-embed-or-floating` — float/embed toggle
- [ ] `edit-scrollback` — open scrollback in `$EDITOR`

### Deprecated / Removed

- ~~Mosh Integration~~: Removed due to Android binary execution restrictions and complexity.
- ~~Mosh Output Bridge~~: Removed.
