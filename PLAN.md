# Zelland Migration Plan: Native Android → Tauri (Svelte 5 + Rust)

This document tracks the migration of Zelland from a native Android application to a cross-platform Tauri v2 application.

## Overview
- **Target Platforms:** Android, Linux Desktop, iOS (future)
- **Tech Stack:** Tauri v2, Svelte 5 (Runes), Rust, Tailwind CSS, xterm.js
- **Architecture:** Hybrid SPA (Single Webview) with Rust Backend

## Milestones

### Phase 1: Scaffolding & Setup
- [ ] **Archive Legacy Code**: Move existing Android code to `legacy_android/` to clear root for Tauri.
- [ ] **Initialize Tauri Project**: Scaffold new project with Svelte 5 + TypeScript.
- [ ] **Configure Mobile**: Run `tauri android init` and configure capabilities.
- [ ] **Install Dependencies**:
    - Rust: `russh`, `tokio`, `prost`, `tauri-plugin-notification`, `tauri-plugin-store`.
    - JS: `xterm`, `xterm-addon-fit`, `svelte`, `tailwindcss`, `lucide-svelte`.
- [ ] **Setup Directory Structure**: Establish `src-tauri/` and `src/` layout.

### Phase 2: Core Logic (Rust Backend)
- [ ] **Port SSH Manager**: Create `src-tauri/src/ssh.rs` replacing `SSHConnectionManager.kt`.
    - Implement `connect`, `disconnect`, `exec`.
    - Implement PTY allocation for interactive sessions.
- [ ] **Implement Daemon Bridge**: Create `src-tauri/src/daemon.rs`.
    - WebSocket client (`tokio-tungstenite`) to talk to `zellandd`.
    - Protobuf parsing (`prost`) for `zelland.proto`.
- [ ] **Event System**: Setup Rust-to-Frontend event emission (e.g., `ssh-output`, `open-tab`).

### Phase 3: Android Integrations (Mobile Features)
- [ ] **Intent Handling (Shared URLs)**:
    - Update `AndroidManifest.xml` with `ACTION_SEND` intent filter.
    - Create Kotlin Plugin (`IntentPlugin.kt`) to intercept `onNewIntent`.
    - Emit `intent://received` event to Tauri.
- [ ] **Platform Notifications**:
    - Integrate `tauri-plugin-notification`.
    - Wire Daemon "Notification" Protobuf messages to system notifications.

### Phase 4: Frontend & UI (Svelte 5)
- [ ] **State Management**:
    - Create `src/lib/stores/session.svelte.ts` using Runes for Tab/Session state.
- [ ] **Terminal Component**:
    - Build `Terminal.svelte` wrapping `xterm.js`.
    - Implement `ResizeObserver` and `xterm-addon-fit`.
- [ ] **Virtual Keyboard**:
    - Port `KeySequenceHelper.kt` logic to TypeScript (`key-mapper.ts`).
    - Build `VirtualKeyboard.svelte` (ModBar + AlphaGrid) with CSS transitions.
- [ ] **Main Layout**:
    - Implement SPA Tab View (Swipeable tabs for Terminal/Viewer).
    - Create `Viewer.svelte` for Images/Markdown.

### Phase 5: Testing & Migration
- [ ] **Unit Tests**:
    - Rust: Test SSH config parsing and logic.
    - TS: Vitest for Key Mapper and Session Store.
- [ ] **E2E Tests**: Playwright tests for Tab switching.
- [ ] **Manual Verification**: Verify Intent handling on device.

## Task Log
- [ ] Created `PLAN.md`
