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
- [ ] **FIDO SSH & Biometrics (In Progress)**
    - See `DESIGN_FIDO_SSH.md` for full design. Using **Approach B (Biometric-gated Standard Keys)**: ed25519 keys encrypted at rest, decrypted via biometric auth before SSH handshake.

    #### Implemented (needs tests)
    - [x] `KeyManager` trait with `generate_key`, `list_identities`, `delete_identity`, `sign` (`keystore.rs:14-29`).
    - [x] `StandardKeyManager`: ed25519 key gen via `russh_keys`, PEM file storage, JSON metadata (`keystore.rs:31-101`).
    - [x] `AndroidKeyManager` stub: delegates to `StandardKeyManager`, placeholder for biometric gating (`keystore.rs:104-151`).
    - [x] `AuthMethod::Key` variant in `SshConfig` (`ssh.rs:15`).
    - [x] Public key auth flow in `SshManager::connect` and `run_command` — reads `.priv` file, calls `authenticate_publickey` (`ssh.rs:84-91`, `ssh.rs:149-156`).
    - [x] Kotlin `KeyStoreManager` — AES key generation in Android Keystore, cipher retrieval (`gen/android/.../KeyStoreManager.kt`).
    - [x] Kotlin `BiometricManager` — `BiometricPrompt` with strong auth and callback interface (`gen/android/.../BiometricManager.kt`).
    - [x] SSH key delete functionality (UI + backend).

    #### Remaining: Rust Backend
    - [ ] **Encrypt private keys at rest**: After generating ed25519 key, encrypt the `.priv` file using a key derived from Android Keystore (AES). On Linux, use `libsecret` or passphrase-based encryption.
    - [ ] **Implement `StandardKeyManager::sign()`**: Load private key from PEM, perform ed25519 signature, return bytes. Currently returns `"not implemented"` error (`keystore.rs:100`).
    - [ ] **Wire up biometric decryption in `AndroidKeyManager`**: On `sign()` or key load, trigger `BiometricPrompt` via JNI/Tauri event, await result on a `oneshot` channel, then decrypt the private key in memory. Currently returns `"not fully wired up"` error (`keystore.rs:149`).
    - [ ] **Implement `AuthMethod::PrivateKey`**: Support user-supplied private key paths (currently returns `"not implemented"` in both `connect` and `run_command` — `ssh.rs:82-83`, `ssh.rs:147`).
    - [ ] **Zeroize key material**: Use the `zeroize` crate to wipe decrypted private keys from memory after SSH handshake completes.
    - [ ] **Deduplicate auth logic**: `connect()` and `run_command()` duplicate the entire auth match block (`ssh.rs:76-92` vs `ssh.rs:141-157`). Extract into a shared `authenticate()` helper.

    #### Remaining: Android / Kotlin Bridge
    - [ ] **JNI bridge for biometric auth**: Expose `BiometricManager.authenticate()` to Rust via JNI or Tauri plugin command. Return crypto object (cipher) on success.
    - [ ] **JNI bridge for Keystore encryption**: Expose `KeyStoreManager.getCipher()` to Rust so private key encryption/decryption can be triggered from the Rust side.
    - [ ] **Async biometric flow**: Implement a Tauri event or `oneshot` channel pattern so the SSH handshake (background thread) can pause, request biometric auth on the UI thread, and resume after success/failure.

    #### Remaining: Frontend (Svelte)
    - [ ] **Key management UI** (`Settings > Keys`): List identities, generate new identity (with label input), copy public key to clipboard, delete identity.
    - [ ] **Connection form**: Add auth method selector (Password / Key) and key picker dropdown to session creation/edit form.
    - [ ] **Biometric prompt trigger**: When connecting with `AuthMethod::Key` on Android, listen for Tauri event requesting biometric auth and invoke the native prompt.

    #### Remaining: Tests (**no tests exist for any keystore/SSH-key functionality**)
    - [ ] **Unit: `StandardKeyManager::generate_key`** — verify ed25519 key pair is created, `.priv` and `.json` files written, `KeyIdentity` fields populated correctly.
    - [ ] **Unit: `StandardKeyManager::list_identities`** — verify listing with 0, 1, and multiple keys; verify corrupt `.json` files are skipped gracefully.
    - [ ] **Unit: `StandardKeyManager::delete_identity`** — verify both `.priv` and `.json` are removed; verify no error when files are already missing.
    - [ ] **Unit: `KeyIdentity` serde roundtrip** — serialize to JSON and deserialize back, verify all fields preserved.
    - [ ] **Unit: `SshConfig` with `AuthMethod::Key`** — verify serialization includes `key_id` field.
    - [ ] **Integration: Key-based SSH auth** — generate key, write to temp dir, verify `authenticate_publickey` path loads key correctly (mock SSH server or verify key decode).
    - [ ] **Integration: `AuthMethod::PrivateKey`** — once implemented, test with user-supplied key path.
    - [ ] **Android: `KeyStoreManager`** — instrumentation test for `generateBiometricKey()` and `getCipher()`.
    - [ ] **Android: `BiometricManager`** — instrumentation test for prompt lifecycle and callback.
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

### Deprecated / Removed
- ~~Mosh Integration~~: Removed due to Android binary execution restrictions and complexity.
- ~~Mosh Output Bridge~~: Removed.