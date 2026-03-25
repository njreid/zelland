# WGPU + Ghostty Migration Plan

This document outlines the strategic migration of `zelland`'s terminal emulator from `alacritty_terminal` + `xterm.js` to `libghostty-vt` + `wgpu` native Android rendering.

## Status: COMPLETED ✅

All phases of the migration have been successfully implemented and validated on Android hardware/emulator.

## Phases

### Phase 0: Testing Harness (Dinghy) ✅
*Goal: Enable rapid "Plan -> Act -> Validate" cycles directly on Android hardware/emulator.*

1. **Dinghy Configuration:** Completed. `dinghy.toml` configured for Android.
2. **Infrastructure Validation:** Completed. `test_android_connectivity` passes.
3. **Ghostty Mock Suite:** Completed. `tests/ghostty_vt_test.rs` validates the FFI wrapper and render state.

### Phase 1: Hybrid Mode (`libghostty-vt` backend + `xterm.js` frontend) ✅
*Goal: Swap the "brain" while keeping the "eyes" the same to validate the VT engine.*

1. **Zig Toolchain Integration:** Completed in `src-tauri/build.rs`.
2. **Rust FFI Bindings:** Completed using `bindgen` in `build.rs`.
3. **Backend Refactor:** `TerminalSession` now uses `GhosttyTerminalWrapper`.
4. **Validation:** Complex TUI apps render correctly via ANSI snapshots sent to `xterm.js`.

### Phase 2: Native Surface & wgpu Foundation ✅
*Goal: Bypass the WebView and render directly to the screen using Rust and the GPU.*

1. **Android SurfaceView:** Implemented in `MainActivity.kt`.
2. **wgpu Setup:** Initialized in `src-tauri/src/renderer.rs`.
3. **Texture Atlas Engine:** `glyphon` integrated for high-performance monospace text rendering.
4. **Validation:** Static "Hello World" terminal grid rendering confirmed.

### Phase 3: Full Integration (The Ghostty Way) ✅
*Goal: Connect the Ghostty brain directly to the wgpu eyes.*

1. **Render State Loop:** `render_native` implemented in `TerminalSession` and called by `SshManager`.
2. **Damage Tracking:** Ghostty row-level dirty tracking implemented in `GhosttyRenderStateWrapper` and `Renderer`.
3. **Native Touch-to-Mouse Emulation:** 
    - JNI bridge `passTouchToRust` implemented in `MainActivity.kt` and `renderer.rs`.
    - Ghostty `GhosttyMouseEncoder` used to translate Android gestures to SGR mouse sequences.
4. **Virtual Viewport & Zero-Scrollback Optimization:** 
    - `max_scrollback` set to 0.
    - Native `wgpu` buffer represents only the visible viewport, minimizing memory footprint.
5. **Cleanup:** 
    - `xterm.js` and `@xterm/*` dependencies removed from `Terminal.svelte`.
    - WebView made transparent to reveal the native `SurfaceView`.

---

## Technical Details

- **Backend:** `libghostty-vt` (Zig) compiled as static library.
- **Frontend:** `wgpu` + `glyphon` (Rust) for native Android `Surface` rendering.
- **IPC:** JNI used for `Surface` and `MotionEvent` passing.
- **Optimization:** Damage-aware row updates and zero-copy path for cell iterations.
