# Design: FIDO-based SSH with Biometrics

This document outlines the design for securing SSH connections in `zelland` using FIDO2/U2F keys (`ed25519-sk`) backed by on-device biometrics (Android KeyStore/BiometricPrompt), replacing the need for Mosh or static passwords.

## Goals

1. **Eliminate Passwords**: Use cryptographic keys for all SSH authentication.
2. **Biometric Security**: Protect private keys using Android's Secure Hardware (StrongBox/TEE) and require biometric authorization (Fingerprint/Face) for every use.
3. **Seamless Reconnection**: Make re-establishing connections low-friction while maintaining high security.
4. **Native SSH**: Leverage the existing Rust `russh` implementation.

## Architecture

### 1. Key Management (Android)

Instead of storing raw private keys on disk, we will use the Android Keystore System to generate and store keys that are:

- **Hardware-backed**: Generated inside the Secure Element (SE) or TEE.
- **Non-exportable**: Private key material never leaves the secure hardware.
- **Biometric-bound**: Usage requires user authentication via `BiometricPrompt`.

Since `russh` and standard OpenSSH servers expect standard key formats, we have two approaches:

- **Approach A (Virtual FIDO Token)**: Emulate a FIDO2 hardware token over the SSH protocol. The Android device acts as the security key. This aligns with `ed25519-sk` keys.
- **Approach B (Standard Key + Keystore)**: Generate a standard `ed25519` key pair, encrypt the private key with a Keystore-backed key, and require biometrics to decrypt it for the SSH agent/session.

**Decision:** **Approach A (Virtual FIDO)** is preferred for modern security standards, but **Approach B** is more compatible with standard SSH libraries if FIDO support is missing in `russh`. Given `russh`'s current state, we might need to implement a custom signer that delegates to Android Keystore.

*Refined Approach:* Use `ed25519` keys where the private key is encrypted at rest using a key derived from the Android Keystore (requiring user auth). When connecting, the app prompts for biometrics, decrypts the private key in memory, performs the handshake, and then discards the key from memory.

### 2. Connection Types

#### "Session" Connection

- **Target**: A specific Zellij session on a remote server.
- **Transport**: SSH over TCP (standard).
- **Authentication**:
    1.  **Setup**: App generates a new SSH key pair (Identity).
    2.  **Registration**: User must add the Public Key to the target server's `~/.ssh/authorized_keys`. The app should provide a convenient way to copy/share this key.
    3.  **Connect**:
        -   User taps "Connect".
        -   App shows `BiometricPrompt` ("Unlock SSH Key").
        -   On success, app loads private key and establishes SSH session.
        -   App executes `zellij attach ...`.

#### "Host" Connection

- **Target**: The underlying infrastructure/VPN endpoint.
- **Transport**: WireGuard (UDP).
- **Layering**: SSH runs *inside* the WireGuard tunnel.
- **Authentication**:
  - **WireGuard**: Static keys (configured once).
  - **SSH**: Same FIDO/Biometric flow as "Session", but connecting to the internal WireGuard IP of the host.

### 3. User Experience (UX) flow

#### A. Generating a New Identity

1. User goes to **Settings > Keys**.
2. Taps **"Generate New Identity"**.
3. Promoted for Biometrics/PIN.
4. App generates key pair in secure storage.
5. App displays the **Public Key** (ssh-ed25519 ...) and a "Copy" button.
6. Instructions: "Add this key to your server's `~/.ssh/authorized_keys` file."

#### B. Connecting

1. User taps a **Session** or **Host** in the Sidebar.
2. If the Host uses WireGuard:
   -   App starts WireGuard tunnel (if not active).
3. App initiates SSH connection.
4. **System Biometric Prompt** appears: "Verify identity to connect to [Host Label]".
5. User authenticates (Face/Touch).
6. SSH Handshake completes.
7. Terminal attaches to Zellij.

### 4. Technical Implementation

#### Rust Backend (`src-tauri/src/ssh.rs`)

- Update `SshConfig` to accept a "Key ID" instead of a password.
- Implement a `KeyStore` trait/struct that handles key retrieval.
- On Android: Use JNI (`jni` crate) to call into Kotlin code for `BiometricPrompt` and Keystore operations.
- On Linux: Use native secret service (e.g., `libsecret`) or standard `~/.ssh` agent interaction.

#### Android Layer (Kotlin)

- Implement `KeyStoreManager`:
  - `generateKey(alias: String)`
  - `getSigner(alias: String, biometricPrompt: Boolean)`
- Expose these via JNI or Tauri Plugin command to Rust.

#### FIDO/SK Specifics (`ed25519-sk`)

If we target strictly `ed25519-sk`, the client must behave like a FIDO authenticator.

- **Pros**: Server enforces presence; private key never exists in app memory (generated on hardware token).
- **Cons**: Requires server-side support for `sk` keys; complex to emulate FIDO HID/CTAP2 protocol in software if `russh` doesn't support generic signers well.

**Recommendation**: Start with **Biometric-gated Standard Keys** (Approach B refined). It provides identical UX (Biometric -> Connect) and high security (key encrypted by TEE) without requiring specific FIDO hardware emulation or server config beyond standard pubkey auth.

## Next Steps

1. Create `src-tauri/src/keystore.rs` interface.
2. Implement Android JNI bridge for Biometrics.
3. Update SSH connect flow to use the Keystore signer.
