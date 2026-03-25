# WGPU + Ghostty implementation critique

Overall, the migration is partway there, but the repo does not yet match the plan's "COMPLETED" claim.

## What lines up with the plan

- `TerminalSession` is backed by Ghostty now, with `max_scrollback: 0` set in `src-tauri/src/ghostty.rs:20`.
- A native renderer exists and `glyphon`/`wgpu` are wired in at `src-tauri/src/renderer.rs:20`.
- The terminal web component no longer renders ANSI into the DOM in `src/lib/components/Terminal.svelte:52`.

## Gaps vs the plan

- The plan claims row-level damage tracking is implemented, but `Renderer::draw_ghostty_state` still rebuilds the entire text buffer on any dirty frame and even calls this out in a comment at `src-tauri/src/renderer.rs:233`.
- The native path still pays for the old hybrid path: `ssh.rs` calls both `render_native()` and `render_viewport()` on every flush, then sends a viewport payload that `Terminal.svelte` ignores. See `src-tauri/src/ssh.rs:273` and `src/lib/components/Terminal.svelte:54`.
- `xterm` dependencies were not actually removed from the app package. They are still present in `package.json:24`.
- The Android implementation described in the plan is hard to verify from the repo because there is no checked-in `MainActivity.kt` or Android Kotlin source here.
- Surface sizing is still placeholder-based. `passSurfaceToRust` forces `1080x2400` and renders a debug string instead of using real surface dimensions/lifecycle events at `src-tauri/src/renderer.rs:351`.
- Touch-to-mouse translation is still based on duplicated fixed cell sizes (`24x32`) in both Rust and Svelte, not measured renderer metrics. See `src-tauri/src/terminal.rs:47`, `src-tauri/src/renderer.rs:17`, and `src/lib/components/Terminal.svelte:16`.
- The current desktop test path is broken: `go-task test` fails on Linux because `ndk-sys` is unconditional in `src-tauri/Cargo.toml:61` and `renderer.rs` is not Android-gated.

## Refactoring I would recommend

- Split `src-tauri/src/renderer.rs` into a platform-neutral terminal rendering facade and an Android-only JNI/surface backend behind `#[cfg(target_os = "android")]`. This should also move `ndk`/`ndk-sys` into Android-only dependencies.
- Make the rendering mode explicit: either remove the old ANSI viewport path entirely, or keep it as a deliberate fallback feature flag. Right now the code does both and gets the cost of both.
- Turn damage tracking into a real row cache. Keep per-row text/style buffers and only rebuild dirty rows instead of recreating one big `String` every frame.
- Replace duplicated `24x32` constants with one source of truth negotiated from the renderer/font metrics, then feed those values to resize and mouse-hit testing.
- Move the debug-first surface bootstrap (`Hello from Zelland...`, hardcoded size) out of production code and hook into real surface create/change/destroy events.
- Remove stale migration leftovers in docs and package metadata (`PLAN.md`, `TESTING.md`, `AGENTS.md`, `package.json`) so the repo reflects the current architecture.

## Additional testing I would add

- A host-side `cargo check`/`cargo test` job for Linux so Android-only rendering code cannot break the normal dev/test workflow again.
- Unit tests around `encode_mouse_event` for press/release/right-click and coordinate mapping edge cases in `src-tauri/src/terminal.rs:47`.
- A render-state test that proves only changed rows are prepared after incremental terminal updates; that is the main missing promise from the plan.
- An integration test for resize/surface changes covering orientation changes and surface recreation on Android.
- A regression test for terminal input on the post-xterm UI path, since `Terminal.svelte` no longer owns keyboard input directly.

## Validation note

- I ran `go-task test`; Vitest passed, but the Rust side failed to compile on Linux because `ndk-sys` only supports Android. That failure blocks confidence in the current "completed" status.
