# Testing Strategy & Guidelines

This document details the testing methodologies for the zelland migration to Mosh + WireGuard + Svelte 5. We adopt a **Test-First** approach where possible.

## 1. Networking Core (Rust)

### Strategy
The networking layer (`src-tauri/src/network.rs`) involving WireGuard (GotaTun) and Mosh integration requires isolation from physical network interfaces for reliable testing.

### Test Types
*   **Unit Tests (Rust)**:
    *   **Goal**: Verify packet wrapping/unwrapping and state management without spinning up a full tunnel.
    *   **Tools**: `cargo test`, `tokio-test`.
    *   **Scenarios**:
        *   `test_handshake_init`: Verify correct handshake packet generation.
        *   `test_keepalive`: Ensure keep-alive packets are generated at configured intervals.
        *   `test_peer_routing`: Verify that packets destined for specific IPs are routed to the correct peer logic.
*   **Integration Tests (Mock Tun)**:
    *   **Goal**: Verify the `start_tunnel` command and userspace loop.
    *   **Method**: Implement a `MockTunDevice` trait that captures packets in memory instead of writing to a system interface.
    *   **Scenarios**:
        *   Start tunnel -> Mock receive packet -> Verify Rust logic processes it.
        *   Simulate network error -> Verify reconnection logic.

## 2. Daemon & Project API

### Strategy
Since the daemon (`zlnd`) runs separately, the client-side testing relies on mocking the API responses.

### Test Types
*   **Contract Tests**:
    *   **Goal**: Ensure the shared Protobuf/JSON definitions matches both client and server expectations.
    *   **Method**: A shared `schema_test` suite that serializes a sample `Project` object in Rust (Daemon side) and asserts it can be deserialized in TypeScript (Client side).
*   **Client API Tests (TypeScript)**:
    *   **Goal**: Verify the Svelte API client handles success, failure, and loading states correctly.
    *   **Tools**: `Vitest`, `msw` (Mock Service Worker).
    *   **Scenarios**:
        *   `fetchProjects`: Mock 200 OK response -> Assert store is populated.
        *   `fetchProjects`: Mock 500 Error -> Assert error state in UI.

## 3. Svelte 5 UI Components

### Strategy
Use **Vitest** with `@testing-library/svelte` to test components in isolation. Focus on the reactive state (`$state`, `$derived`) provided by Svelte 5 Runes.

### Test Types
*   **State Logic Tests**:
    *   **Goal**: Verify `project.svelte.ts` logic independent of the UI.
    *   **Scenarios**:
        *   `setProject(id)` updates `currentProject.id`.
        *   `isProjectReady` derivation correctly reflects `status === 'active'`.
*   **Component Tests**:
    *   **Goal**: Verify rendering and user interactions.
    *   **Scenarios**:
        *   **Infinite Ribbon**: Render 4 panes. Verify CSS classes for "snap-align" are present.
        *   **Terminal**: Verify terminal session lifecycle and resize behavior without depending on `xterm.js`.
        *   **Gesture Lock**: Verify that enabling "Gesture Lock" prevents propagation of swipe events.

## 3A. Native Terminal Stack (Ghostty + wgpu)

### Strategy
Treat the native terminal as a mixed Rust/UI integration point. Keep host-side tests focused on Ghostty state handling and protocol encoding, and reserve Android surface validation for smoke tests.

### Test Types
*   **Rust unit tests**:
    *   **Goal**: Verify terminal state transitions and mouse encoding without a device.
    *   **Scenarios**:
        *   Ghostty terminal init/write/resize.
        *   `TerminalSession::encode_mouse_event` emits SGR mouse sequences when mouse tracking is enabled.
        *   Native render loop compiles on Linux hosts even though Android JNI entrypoints are target-gated.
*   **Android smoke tests**:
    *   **Goal**: Validate `Surface` lifecycle, resize/orientation handling, and touch forwarding on hardware/emulator.
    *   **Scenarios**:
        *   Surface create -> first frame rendered.
        *   Orientation change -> terminal resizes without panic.
        *   Touch interaction reaches the SSH session when mouse mode is enabled.

## 4. Mobile & E2E

### Strategy
End-to-End tests on mobile are expensive. We prioritize "Smoke Tests" to verify the critical path.

### Test Types
*   **Manual Verification (Smoke Test)**:
    *   **Flow**:
        1.  Launch App.
        2.  Click "Start Tunnel".
        3.  Verify "Connected" indicator.
        4.  Select a Project.
        5.  Verify Terminal opens and accepts input.
        6.  Swipe right -> Verify README.md loads.

## 5. CI/CD Pipeline Tasks

*   `cargo test`: Runs Rust unit and integration tests.
*   `npm run test`: Runs Vitest for Svelte components and state logic.
*   `npm run check`: TypeScript type checking.
*   `npm run lint`: Code style verification.
