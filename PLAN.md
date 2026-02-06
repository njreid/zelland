# Zelland Implementation Plan: Mosh + WireGuard + Svelte 5

This document tracks the evolution of Zelland into a resilient mobile command center using WireGuard tunneling and Mosh for terminal persistence.

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
- [ ] **Write Tests (Rust)**: Setup unit tests for network packet logic in `src-tauri/src/network.rs`.
- [ ] **WireGuard Integration (GotaTun)**
    - [ ] Add `boringtun` or `gotatun` dependency to `src-tauri`.
    - [ ] Implement userspace packet loop in `src-tauri/src/network.rs`.
    - [ ] Create Tauri command `start_tunnel(config)` to bring up the interface.
    - [ ] **Test:** Verify handshake and keep-alive with a mock peer.
- [ ] **Mosh Integration**
    - [ ] Research: Bundle `mosh-client` binary (Android) vs. link `libmosh` (iOS/Rust).
    - [ ] Implement `src-tauri/src/mosh.rs` to spawn Mosh inside the WG tunnel.
    - [ ] Bridge Mosh output to frontend events (`mosh-data`).
    - [ ] **Test:** Ensure Mosh traffic flows through the userspace tunnel.
- [ ] **Run Tests**: Execute `cargo test` to verify networking logic.

### Phase 2: Daemon & Project API
- [ ] **Write Tests (API)**: Create mock daemon responses and client contract tests.
- [ ] **Daemon Updates (zellandd)**
    - [ ] Define `Project` KDL schema (host, session_name, root_path).
    - [ ] Implement REST endpoints:
        - `GET /projects`: List available projects.
        - `POST /projects/activate`: Start/Attach Zellij session.
        - `GET /fs/read`: Read file content (for Markdown previews).
- [ ] **Client API Bridge**
    - [ ] Implement local HTTP proxy in Tauri (`axum` or similar) to forward requests through WG.
    - [ ] Create Svelte API client (`src/lib/api.ts`) to talk to the local proxy.
- [ ] **Run Tests**: Execute client API contract tests.

### Phase 3: Svelte 5 UI Overhaul
- [ ] **Write Tests (UI)**: Setup Component tests for Layout and State logic.
- [ ] **Reactive State**
    - [ ] Create `src/lib/stores/project.svelte.ts` using Runes for project/connection state.
- [ ] **Infinite Ribbon Layout**
    - [ ] Implement `src/routes/+page.svelte` with CSS Snap Scroll.
    - [ ] Create `Pane.svelte` wrapper component.
- [ ] **Markdown Previews**
    - [ ] Build `MarkdownPane.svelte` fetching content from `currentProject`.
    - [ ] Integrate a markdown renderer (e.g., `markdown-it` or `marked`).
- [ ] **Refine Terminal**
    - [ ] Update `Terminal.svelte` to consume Mosh stream instead of raw SSH.
    - [ ] Implement "Gesture Lock" to prevent scroll conflicts.
- [ ] **Run Tests**: Execute `npm run test` for UI components.

### Phase 4: Mobile Specifics
- [ ] **Android VPN Service**
    - [ ] Update `AndroidManifest.xml` for `BIND_VPN_SERVICE`.
    - [ ] Implement Kotlin side to allow userspace Tun via Tauri.
- [ ] **Background Persistence**
    - [ ] Ensure WG tunnel stays alive when app is backgrounded.

### Phase 5: Verification
- [ ] **Unit Tests**: Rust networking logic, Svelte state logic.
- [ ] **E2E**: Verify full flow: App Open -> Tunnel Up -> Project List -> Connect -> Terminal Active.
