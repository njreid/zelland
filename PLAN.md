# zelland Implementation Plan: Mosh + WireGuard + Svelte 5

This document tracks the evolution of zelland into a resilient mobile command center using WireGuard tunneling and Mosh for terminal persistence.

## Overview
- **Target Platforms:** Android, Linux Desktop
- **Tech Stack:** Tauri v2, Svelte 5 (Runes), Rust (GotaTun), Mosh, xterm.js
- **Architecture:** Userspace WireGuard Tunnel -> Mosh over UDP -> Local Svelte 5 UI

## Completed Foundation
- [x] Tauri v2 Project Scaffolding
- [x] Basic Svelte 5 + Tailwind Setup
- [x] Initial SSH/Terminal Components (to be refactored)
- [x] Basic Android Intent Handling

## Milestones

### Phase 1: Networking Core (Rust)
- [x] **Write Tests (Rust)**: Setup unit tests for network packet logic in `src-tauri/src/network.rs`.
- [x] **WireGuard Integration (GotaTun)**
    - [x] Add `boringtun` or `gotatun` dependency to `src-tauri`.
    - [x] Implement userspace packet loop in `src-tauri/src/network.rs`.
    - [x] Create Tauri command `start_tunnel(config)` to bring up the interface.
    - [x] **Test:** Verify handshake and keep-alive with a mock peer.
- [x] **SSH Integration (Native)**
    - [x] Implement SSH backend in Rust (`src-tauri/src/ssh.rs`).
    - [x] Basic connection and authentication (password).
    - [x] PTY management (`russh` pty request).
    - [x] `xterm.js` <-> `russh` data pipe.
- [x] **FIDO SSH & Biometrics**
    - See `DESIGN_FIDO_SSH.md` for full design. Using **Approach B (Biometric-gated Standard Keys)**: ed25519 keys encrypted at rest, decrypted via biometric auth before SSH handshake.

    #### Rust Backend
    - [x] `KeyManager` trait with `generate_key`, `list_identities`, `delete_identity`, `sign`.
    - [x] `StandardKeyManager`: ed25519 key gen via `ssh_key` crate, OpenSSH PEM storage, JSON metadata.
    - [x] `StandardKeyManager::sign()`: Load key, decrypt, ed25519 signature via `ed25519-dalek`.
    - [x] **Encrypt private keys at rest**: OpenSSH bcrypt-pbkdf encryption with auto-generated master passphrase. `Zeroizing<String>` for key material.
    - [x] `AuthMethod::Key` variant in `SshConfig` with keystore-managed key auth.
    - [x] `AuthMethod::PrivateKey`: User-supplied private key path with optional passphrase.
    - [x] **Deduplicated auth logic**: Shared `load_private_key()` and `authenticate()` helpers.

    #### Android / Kotlin Bridge
    - [x] `AndroidKeyManager`: Biometric-gated signing via Tauri event + `oneshot` channel pattern.
    - [x] `BiometricRequest`/`BiometricResponse` types with global pending request registry.
    - [x] Kotlin `KeyStoreManager` — AES key gen, encrypt/decrypt data, biometric-bound cipher.
    - [x] Kotlin `BiometricManager` — `BiometricPrompt` with strong auth and callback.
    - [x] Kotlin `MainActivity` — `authenticateAndDecrypt`, `encryptWithBiometricKey` JNI methods.

    #### Frontend (Svelte)
    - [x] **Key management UI**: Label input for generation, copy public key (with ssh-ed25519 prefix), delete.
    - [x] **Connection form**: Auth method selector (Password / SSH Identity / Private Key File) with conditional fields.
    - [x] **Biometric prompt trigger**: Event listener for `biometric-request`, auto-approve on desktop.
    - [x] **Session persistence**: `key_id` and `private_key_path` persisted in store.
    - [x] **Complete SshConfig**: All fields sent to backend (`private_key_path`, `private_key_passphrase`, `key_id`).

    #### Tests (26 total)
    - [x] `StandardKeyManager::generate_key` — file creation, ed25519 validity, encryption at rest.
    - [x] `StandardKeyManager::list_identities` — empty, multiple keys, corrupt JSON skip.
    - [x] `StandardKeyManager::delete_identity` — file removal, nonexistent key OK.
    - [x] `StandardKeyManager::sign()` — valid signature with ed25519-dalek verification, missing key error.
    - [x] `KeyIdentity` serde roundtrip.
    - [x] `SshConfig` serde for `AuthMethod::Key` and `AuthMethod::PrivateKey`.
    - [x] Private key file decode roundtrip (encrypted/decrypted).
    - [x] Master passphrase creation and reuse.
    - [x] `load_decrypted_key` correctness.
    - [x] Biometric bridge: success, failure, unknown request, request/response serde.
    - [x] SSH module: key loading from file, missing path, bad file, AuthMethod serde variants.
    - [ ] **Android instrumentation tests** (requires device): `KeyStoreManager`, `BiometricManager`.
- [x] **Run Tests**: Execute `cargo test` to verify networking logic.

### Phase 2: Daemon & Project API
- [x] **Write Tests (API)**: Create mock daemon responses and client contract tests.
- [x] **Daemon Updates (zellandd)**
    - [x] Define `Project` KDL schema (host, session_name, root_path).
    - [x] Implement REST endpoints:
        - `GET /projects`: List available projects.
        - `POST /projects/activate`: Start/Attach Zellij session.
        - `GET /fs/read`: Read file content (for Markdown previews).
- [x] **Client API Bridge**
    - [x] Implement local HTTP proxy in Tauri (`axum` or similar) to forward requests through WG.
    - [x] Create Svelte API client (`src/lib/api.ts`) to talk to the local proxy.
- [x] **Run Tests**: Execute client API contract tests.

### Phase 3: Svelte 5 UI Overhaul
- [x] **Write Tests (UI)**: Setup Component tests for Layout and State logic.
- [x] **Reactive State**
    - [x] Create `src/lib/stores/app.svelte.ts` using Runes for project/connection state.
- [x] **Infinite Ribbon Layout**
    - [x] Implement `src/routes/+page.svelte` with CSS Snap Scroll.
    - [x] Create `Pane.svelte` wrapper component (Merged into Page/MarkdownPane).
- [x] **Markdown Previews**
    - [x] Build `MarkdownPane.svelte` fetching content from `currentProject`.
    - [x] Integrate a markdown renderer (e.g., `markdown-it` or `marked`).
- [x] **Refine Terminal**
    - [x] Update `Terminal.svelte` to consume SSH stream.
    - [x] Implement "Gesture Lock" to prevent scroll conflicts.
- [x] **Run Tests**: Execute `npm run test` for UI components.

### Phase 4: Mobile Specifics
- [x] **Android VPN Service**
    - [x] Update `AndroidManifest.xml` for `BIND_VPN_SERVICE`.
    - [x] Implement Kotlin side to allow userspace Tun via Tauri.
- [x] **Background Persistence**
    - [x] Ensure WG tunnel stays alive when app is backgrounded.

### Phase 5: Verification
- [x] **Unit Tests**: Rust networking logic, Svelte state logic.
- [x] **E2E**: Verify full flow: App Open -> Tunnel Up -> Project List -> Connect -> Terminal Active.

### Phase 6: Daemon Migration (Go → Rust) & Annotation System

Rewrite the companion daemon (`zellandd`) from Go to Rust to enable first-class YJS CRDT support via the `yrs` crate. The Go daemon is ~1,067 lines of source code. See `DAEMON_DESIGN.md` for architecture and `ANNOTATION_DESIGN.md` for the annotation system design.

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
- [x] **`zellandd`** (`main.rs`): Entry point with `clap` flags (`--config`, `--port`).
- [x] **`zelland`** (separate `[[bin]]`): CLI tool for `show` and `md` trigger commands via localhost HTTP.
- [x] **Integration tests:** Full API flow, WebSocket connect + ping, trigger from loopback.

#### 6H: YJS Integration
- [ ] Add `yrs` crate dependency.
- [ ] **YJS document manager**: One `yrs::Doc` per annotation file, keyed by path.
- [ ] **Sync endpoint** (`/annotations/{filepath}/sync`): YJS WebSocket sync protocol using `y-sync` crate.
- [ ] **Persistence**: Load `.ann.kdl` → initialize YJS doc on cold start. Flush YJS state → `.ann.kdl` on debounced timer (5s) and on shutdown.
- [ ] **KDL ↔ YJS bridge**: Serialize YJS annotation map to KDL format, and vice versa.
- [ ] **Tests:** YJS doc creation, sync message handling, KDL↔YJS round-trip, concurrent edits.

#### 6I: Integration & Cutover
- [ ] Verify Tauri client (`daemon.rs`) works against new Rust daemon (same proto, same REST API).
- [x] Update `Taskfile.yml` `dev:daemon` to build/run Rust daemon.
- [ ] Remove Go `daemon/` directory.
- [ ] Update `DAEMON_DESIGN.md` and `ANNOTATION_DESIGN.md`.

### Future: Zellij Action Buttons
Expose `zellij -s $SESSION action <action>` via `runZellijAction()` as UI buttons (TopBar for desktop, VirtualKeyboard for mobile).

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