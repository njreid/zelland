# MOSH_DESIGN.md: Wireguard + Mosh + Svelte 5 Architecture

This document outlines the design for the next iteration of the zelland mobile shell, moving from a standard SSH-based approach to a highly resilient architecture using **Wireguard** for the tunnel and **Mosh** for terminal persistence, all wrapped in a **Svelte 5** layout.

## 1. Objective
Build a "Mobile Command Center" that remains connected across network switches (Wi-Fi to 5G) and provides a seamless developer experience with integrated project management and documentation viewing.

## 2. Architecture Overview

### A. Network Layer (Wireguard + Mosh)
- **Tunneling**: Use `GotaTun` (Mullvad's Rust Wireguard fork) in userspace. 
- **Encapsulation**: Mosh runs *inside* the Wireguard tunnel.
    - **Single Port**: Only UDP 51820 needs to be open on the server.
    - **Statelessness**: Wireguard handles IP roaming at the packet level; Mosh handles terminal state synchronization at the application level.
- **MTU**: Fixed at **1280 bytes** to prevent fragmentation over mobile carriers.

### B. Backend Layer (Tauri + System Daemon)
- **Tauri (Client)**: 
    - Manages the `GotaTun` packet loop in Rust.
    - Provides a local HTTP proxy to bridge the Svelte WebView to the remote Daemon via the WG tunnel.
    - Spawns and manages the `mosh-client` (via FFI on iOS, subprocess on Android).
- **zelland Daemon (Server)**:
    - Maintains a KDL-based store of "Projects".
    - Orchestrates **Zellij** sessions.
    - Provides a REST API for project discovery, file reading (Markdown), and session activation.

### C. Frontend Layer (Svelte 5 + xterm.js)
- **State Management**: Uses Svelte 5 **Runes** (`$state`, `$derived`, `$effect`) for reactive project status and connection health.
- **UI Layout**: An "Infinite Ribbon" using CSS Scroll Snap.
- **Terminal**: `xterm.js` rendering the Mosh output.

## 3. The "Project" Concept
A project is defined by:
- **Remote Host**: IP/Domain and User credentials (SSH Keys).
- **Zellij Session**: The unique identifier for the terminal multiplexer session.
- **Root Directory**: The absolute path on the remote host where the project resides.
- **Metadata**: Associated tags or custom environment variables.

## 4. Layout: The Infinite Ribbon
The UI consists of a horizontal snap-scroll container:
1. **Pane 0**: Terminal (`xterm.js`) connected to the Zellij session.
2. **Pane 1**: `README.md` (Rendered Markdown).
3. **Pane 2**: `PLAN.md` (Rendered Markdown).
4. **Pane 3**: `DESIGN.md` (Rendered Markdown).

Svelte 5 handles the visibility and focus management to ensure the terminal only consumes resources/input when active.

---

## 5. Testing Strategy

### A. Networking & Tunneling
- **Unit (Rust)**: Test `GotaTun` handshake logic and packet wrapping/unwrapping using mock peers.
- **Integration**: Simulate network drops (killing interface) and verify that the Wireguard state recovers and Mosh session remains alive.
- **MTU Verification**: Use `ping -s` with various sizes over the tunnel to ensure 1280 is respected without fragmentation.

### B. System Daemon API
- **Mocking**: Create a mock daemon in Rust to test the client's discovery and activation flow without a real server.
- **Contract Testing**: Ensure the Protobuf/JSON schema between Client and Daemon remains in sync.

### C. Frontend (Svelte 5 + xterm.js)
- **Component Tests**: Use Vitest + Svelte Testing Library to verify that `$state` updates correctly when a project is selected.
- **UI/UX (Playwright)**:
    - Verify horizontal scroll-snap behavior.
    - Test "Swipe Lock" functionality to prevent accidental pane switches while typing.
    - Ensure `xterm.js` resizes correctly when the virtual keyboard appears/disappears.

### D. End-to-End (E2E)
- **Device Testing**: Deploy to Android/iOS and verify the "Shared Intent" flow:
    - User shares a URL → App opens → Wireguard connects → Mosh attaches to Zellij → Correct project root is focused.

---

## 6. Clarifying Questions & Design Nuances

1. **Zellij Layouts**: Should the daemon allow specifying a `layout.kdl` per project? (e.g., automatically opening a code editor and a log pane).
2. **Daemon Auth**: Should we implement a "Pairing" flow where the client exchanges a public key with the daemon over an initial SSH handshake to authorize future WG/API requests?
3. **Swipe vs. Terminal**: How do we handle horizontal gestures in the terminal (e.g., `ctrl-alt-left/right` to switch Zellij tabs) vs. the Svelte Ribbon scroll? 
    - *Proposed Solution*: A toggleable "Gesture Lock" in the footer.
4. **Markdown Persistence**: Should the markdown panes allow editing?
    - *Proposed Solution*: Start with Read-Only. If editing is added, use a simple `textarea` or Svelte-based editor that `POST`s changes back to the daemon's `/fs/write` endpoint.

---

## 7. Implementation Roadmap
1. **Infrastructure**: Implement `GotaTun` userspace loop in `src-tauri/src/network.rs`.
2. **Daemon**: Update `zellandd` to support Project KDL storage and REST endpoints.
3. **Frontend**: Build the Svelte 5 Ribbon layout and `$state` managers.
4. **Mosh Integration**: Link `libmosh` (or bundle binary) and wire it to `xterm.js`.
