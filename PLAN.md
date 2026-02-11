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
- [x] **Mosh Integration**
    - [x] Research: Bundle `mosh-client` binary (Android) vs. link `libmosh` (iOS/Rust).
    - [x] Implement `src-tauri/src/mosh.rs` to spawn Mosh inside the WG tunnel.
    - [x] Bridge Mosh output to frontend events (`mosh-data`).
    - [x] **Test:** Ensure Mosh traffic flows through the userspace tunnel.
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
    - [x] Create `src/lib/stores/project.svelte.ts` using Runes for project/connection state.
- [x] **Infinite Ribbon Layout**
    - [x] Implement `src/routes/+page.svelte` with CSS Snap Scroll.
    - [x] Create `Pane.svelte` wrapper component (Merged into Page/MarkdownPane).
- [x] **Markdown Previews**
    - [x] Build `MarkdownPane.svelte` fetching content from `currentProject`.
    - [x] Integrate a markdown renderer (e.g., `markdown-it` or `marked`).
- [x] **Refine Terminal**
    - [x] Update `Terminal.svelte` to consume Mosh stream instead of raw SSH.
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