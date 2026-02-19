# NOTES

- Use PLAN.md to track all milestones and tasks. Update tasks once completed.

- Always check-in your incremental work using jj. Use `jj init git` if needed to start a repository.

- Always look for tests in the canonical format for the languages you're writing and make sure they pass after successful compilation. Make sure TESTING.md is up to date with details about tests implemented and planned.

## New Feature Process

- Always create a feature_DESIGN.md in ./docs/features before starting. Consider whether new types of testing is required.

- Break down the feature into easily completed tasks, add them to a new section in project_root/PLAN.md, and keep the status updated as you build.

## Project Layout

  This is a monorepo with three main components:
`daemon-rs/` — Rust daemon (axum/tokio), the backend server (`zlnd` + `zn` CLI)
`src-tauri/` — Rust Tauri v2 client, bridges the daemon API to the Svelte UI
`src/` — Svelte 5 frontend (SvelteKit, xterm.js, yjs)

## Build System

Use `task` (Taskfile.yml) for all build/test/deploy commands — not raw cargo/npm. You might need to invoke `go-task` binary on this platform.
`task test` runs all test suites (cargo test for both Rust crates + npm test for Svelte).
`task dev` launches daemon + Tauri dev mode in a Zellij session.
`task build` produces release binaries for Linux + Android.

## Rust Conventions (daemon-rs and src-tauri)

Protobuf wire format via `prost 0.14.3` — pinned in both crates for compatibility. The single proto file is `proto/zelland.proto` at the project root.
Config files use KDL format (kdl crate v6). See memory notes for API gotchas.
YJS/CRDT sync uses `yrs 0.21` directly with manual lib0 framing — do not add `y-sync` as a dependency.
Async runtime is tokio; HTTP framework is axum 0.8.
Tests: unit tests in `#[cfg(test)]` modules, integration tests use `axum::ServiceExt::oneshot()` or bind to `127.0.0.1:0`.
Use `tracing` for logging, not `println!` or `log`.

## Svelte Conventions (src/)

Svelte 5 with runes (`$state`, `$derived`, `$effect`) — do not use legacy stores or `$:` reactive statements.
Component tests use vitest + @testing-library/svelte.
Styling: Pico CSS + minimal custom classes in custom.css.

## Secrets & Security

Never commit `.env*` files, `*.keystore`, `google-services.json`, or SSH keys.
The daemon's trigger endpoints are guarded by loopback-only middleware — preserve this pattern for any new endpoints that accept commands.

## Proto / API Changes

Edit `proto/zelland.proto` — both crates compile from this single file.
WebSocket messages are binary protobuf, not JSON. REST endpoints use JSON.
