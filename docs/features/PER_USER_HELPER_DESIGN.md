# Per-User Helper Migration Design

## Goal

Replace the current remote system-style daemon model with a per-user helper app that is installed into `~/bin`, started on demand, and kept current automatically whenever a zelland SSH session connects.

The helper preserves all existing daemon functionality:

- REST API for projects, file reads, assets, annotations, and recent sessions
- WebSocket protobuf channel for live control, notifications, and future actions
- File watching, asset registration, and annotation sync
- `zn` local trigger workflow for `show`, `md`, and `notify`

## Current State

The existing implementation assumes a long-lived daemon listening on `0.0.0.0:8083` and a client that talks directly to `http://<host>:8083` and `ws://<host>:8083/ws`.

Important current coupling points:

- `src/lib/stores/app.svelte.ts` hardcodes daemon access to port `8083`
- `src-tauri/src/daemon.rs` opens direct HTTP and WebSocket connections to the remote host
- `src-tauri/src/ssh.rs` already provides authenticated command execution over SSH, which can be reused for helper bootstrap, version checks, and process startup

## New Model

### 1. Helper Packaging

Build `zlnd` as a standalone helper binary for:

- Linux `x86_64`
- Linux `aarch64`
- macOS `aarch64`
- macOS `x86_64`

Release artifacts should be published through GitHub Actions and attached to tagged releases.

### 2. Helper Installation

On every SSH session establishment:

1. Detect remote OS/arch using SSH.
2. Check whether `~/bin/zlnd` exists.
3. Check the remote helper version.
4. If missing or outdated, copy the matching release artifact to `~/bin/zlnd` and mark it executable.

The helper version should be a build-time version string aligned with the app release/tag so the client can compare local expected version against the remote binary.

### 3. Helper Startup

After install/version validation, ensure the helper is running.

Recommended bootstrap command shape:

```sh
mkdir -p ~/bin ~/.local/state/zelland && nohup ~/bin/zlnd >~/.local/state/zelland/zlnd.log 2>&1 </dev/null &
```

Startup flow:

1. Probe whether the helper HTTP endpoint is reachable on the expected port.
2. If unreachable, launch via `nohup` so it survives SSH disconnect.
3. Re-probe until healthy or timeout.

### 4. Transport Expectations

The helper remains a per-user process, but it continues to expose the same remote HTTP and WebSocket API surface on port `8083` so the existing client contract stays intact.

Runtime behavior:

- keep the current `http://<host>:8083` and `ws://<host>:8083/ws` access model
- keep trigger endpoints loopback-only for `zn` and local remote-host tooling
- move lifecycle and version management into the SSH bootstrap path rather than changing the API transport itself

### 5. Client Responsibilities

The Tauri SSH layer should own helper lifecycle management:

- resolve remote platform
- compare expected helper version
- install/update helper binary
- start helper if missing
- establish the API/WebSocket path used by existing UI features
- bootstrap at the host level as well as the session level so project discovery works before any zellij session has been created

### 6. Session Startup Behavior

When zelland creates a brand-new zellij session for a project, the initial shell should start in that project's root directory. Existing sessions should still be attached normally without changing their current working directory.

This keeps helper orchestration close to existing SSH session setup and avoids pushing install logic into Svelte.

## Proposed Implementation Phases

### Phase A: Release and Versioning

- stamp `zlnd` with semantic/app version metadata
- add GitHub workflow to build helper artifacts for target platforms
- attach artifacts to releases in a predictable naming format

### Phase B: Remote Bootstrap

- add remote OS/arch detection in `src-tauri/src/ssh.rs`
- add helper version probe command
- add artifact upload/install path into `~/bin`
- add helper health check and `nohup` startup

### Phase C: Client Integration Preservation

- preserve direct `http://<host>:8083` and `ws://<host>:8083/ws` client access
- bootstrap the helper before those calls are made
- keep all existing app features working without frontend behavior regressions

### Phase D: Hardening

- improve log path/config path defaults for per-user execution
- preserve annotation/watcher behavior after reconnects and helper restarts
- verify notifications still arrive after the interactive SSH session drops

## Packaging Decision

Publish separate helper artifacts for `x86_64-apple-darwin` and `aarch64-apple-darwin` rather than a universal macOS binary. Apple Silicon is the primary mac target, but Intel macOS support remains useful for older hosts.
