# WGPU + Ghostty Remediation Design

This document captures the follow-up work required to make the completed WGPU + Ghostty migration match the intended architecture and keep the repo healthy on non-Android hosts.

## Goals

- Restore `go-task test` on Linux hosts.
- Remove the old hybrid render loop cost from the native path.
- Keep Android-specific JNI and NDK wiring from leaking into host builds.
- Add regression coverage for Ghostty mouse encoding and native terminal behavior.
- Align docs and package metadata with the current terminal stack.

## Scope

1. Move Android-only dependencies behind target-specific Cargo sections.
2. Fix the current `glyphon` integration to match the crate API in use.
3. Stop generating and shipping the ignored ANSI viewport payload on each native frame.
4. Remove stale main-app `xterm` npm dependencies.
5. Add focused Rust tests for mouse encoding and keep testing docs current.

## Non-goals

- Rewriting the renderer to support true per-row GPU updates in this pass.
- Reworking Android Kotlin lifecycle code that is not checked into this repo snapshot.
- Designing a new desktop terminal input model.

## Implementation notes

- Keep `Renderer` available to the crate, but gate JNI entrypoints and Android NDK imports with `#[cfg(target_os = "android")]`.
- Use `glyphon::Cache` as the source of truth for viewport creation, since `Viewport::new` requires a cache rather than a texture format.
- Preserve the existing Tauri channel shape for now, but stop sending rendered viewport bytes once native rendering is active.
- Prefer additive tests in Rust rather than broad snapshots so the suite remains portable.

## Testing

- `go-task test`
- Targeted Rust tests for `TerminalSession::encode_mouse_event`
