# WGPU + Ghostty implementation critique

The current codebase is much closer to the migration plan than the plan document implies, but it still falls short of the plan's "COMPLETED" status in a few important ways.

## What is genuinely in place

- The terminal backend is now Ghostty-based, with zero local scrollback configured in `src-tauri/src/ghostty.rs:20`.
- Native rendering is real: `wgpu` + `glyphon` are wired up in `src-tauri/src/renderer/mod.rs:28` and `Terminal.svelte` no longer paints ANSI into the DOM at `src/lib/components/Terminal.svelte:51`.
- The old viewport channel is mostly gone. `SshChannelMsg` only emits `Closed` now in `src-tauri/src/ssh.rs:20`, so the Phase 1 bridge has largely been removed.
- Android JNI entry points for surface attach/destroy/resize/touch exist in `src-tauri/src/renderer/android.rs:18`.

## Main implementation gaps

- The biggest correctness bug is in `src-tauri/src/terminal.rs:127`: `render_native()` clears `self.dirty` before updating/rendering, and it also resets Ghostty's dirty flag even when no renderer exists. If the first flush happens before the Android surface is ready, the frame is dropped and nothing forces a redraw later.
- Damage tracking is only partial. `src-tauri/src/renderer/mod.rs:381` caches rows, but any changed row still causes the renderer to rebuild one large rich-text buffer and re-run shaping for the entire viewport. That is row diffing, not end-to-end row-level rendering.
- Styling support is incomplete. `build_row_runs()` in `src-tauri/src/renderer/mod.rs:494` preserves fg/bold/italic, but background colors, underline, cursor treatment, and selection/inversion rendering are still missing. `render()` also clears to hard-coded magenta at `src-tauri/src/renderer/mod.rs:253`, which makes the native path look like a debug surface rather than a terminal.
- Layout math is still hard-coded in multiple places. `CELL_WIDTH`/`CELL_HEIGHT` live in `src-tauri/src/renderer/mod.rs:15`, while `Terminal.svelte` independently divides by `24` and `32` at `src/lib/components/Terminal.svelte:17` and `src/lib/components/Terminal.svelte:76`. Mouse encoding uses the same assumptions in `src-tauri/src/terminal.rs:58`, so renderer metrics, resize math, and hit testing can drift.
- SSH PTY resize still throws away pixel dimensions. `channel.window_change(cols, rows, 0, 0)` in `src-tauri/src/ssh.rs:245` means the remote side never receives real pixel sizing even though Ghostty locally tracks pixel dimensions.
- The renderer architecture is still single-surface and global. `static RENDERER: Lazy<Mutex<Option<Renderer>>>` in `src-tauri/src/renderer/mod.rs:52` makes multiple independent terminal surfaces difficult and couples every session to one process-wide lock.
- The Android side is not fully reviewable from the repo. The Rust JNI hooks refer to `MainActivity`, but there is no checked-in `MainActivity.kt` under `src-tauri/gen/android/...`, so the plan's Android lifecycle claims cannot be verified end-to-end from source.

## Refactoring notes

- Make frame invalidation reliable first: only clear `TerminalSession.dirty` and Ghostty dirty flags after a frame is successfully prepared for an attached renderer.
- Split `src-tauri/src/renderer/mod.rs` into a terminal rendering core and an Android surface/JNI adapter. Keep the global singleton out of the rendering core so sessions own explicit renderer handles.
- Replace the shared `24x32` guess with measured font metrics from the renderer and flow those values into Svelte resize, Ghostty resize, and mouse-event encoding.
- Upgrade damage handling from cached rows to cached prepared text segments, or separate row buffers, so unchanged rows do not trigger full viewport shaping.
- Finish the visual model: add bg colors, underline, reverse-video, cursor rendering, and remove the hard-coded magenta clear path.
- Move Android-specific lifecycle verification into checked-in Kotlin sources or document exactly where that code lives if it is generated elsewhere.

## Testing and validation

- `go-task test` passes on the current tree: 28 Vitest tests, 29 `src-tauri` tests, 63 daemon tests, and 10 `src-voice` Rust tests all passed.
- The existing tests are still light on the new renderer path. The next useful additions are a dropped-surface redraw regression, a resize/orientation lifecycle test, and a renderer test that proves unchanged rows do not get fully rebuilt.
