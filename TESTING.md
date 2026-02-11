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
Since the daemon (`zellandd`) runs separately, the client-side testing relies on mocking the API responses.

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
        *   **Terminal**: Verify `xterm.js` instance is created/destroyed on mount/unmount.
        *   **Gesture Lock**: Verify that enabling "Gesture Lock" prevents propagation of swipe events.

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
