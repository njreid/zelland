# Zelland Migration Plan: Native Android → Tauri (Svelte 5 + Rust)

This document tracks the migration of Zelland from a native Android application to a cross-platform Tauri v2 application.

## Overview
- **Target Platforms:** Android, Linux Desktop, iOS (future)
- **Tech Stack:** Tauri v2, Svelte 5 (Runes), Rust, Tailwind CSS, xterm.js
- **Architecture:** Hybrid SPA (Single Webview) with Rust Backend

## Milestones

### Phase 1: Scaffolding & Setup
- [x] **Archive Legacy Code**: Move existing Android code to `legacy_android/` to clear root for Tauri.
- [x] **Initialize Tauri Project**: Scaffold new project with Svelte 5 + TypeScript.
- [x] **Configure Mobile**: Run `tauri android init` and configure capabilities.
- [x] **Install Dependencies**:
    - Rust: `russh`, `tokio`, `prost`, `tauri-plugin-notification`, `tauri-plugin-store`.
    - JS: `xterm`, `xterm-addon-fit`, `svelte`, `tailwindcss`, `lucide-svelte`.
- [x] **Setup Directory Structure**: Establish `src-tauri/` and `src/` layout.

### Phase 2: Core Logic (Rust Backend)
- [x] **Port SSH Manager**: Create `src-tauri/src/ssh.rs` replacing `SSHConnectionManager.kt`.
    - [x] Implement `connect`, `disconnect`, `exec`.
    - [x] Implement PTY allocation for interactive sessions.
- [x] **Implement Daemon Bridge**: Create `src-tauri/src/daemon.rs`.
    - [x] WebSocket client (`tokio-tungstenite`) to talk to `zellandd`.
    - [x] Protobuf parsing (`prost`) for `zelland.proto`.
- [x] **Event System**: Setup Rust-to-Frontend event emission (e.g., `ssh-output`, `open-tab`).

### Phase 3: Android Integrations (Mobile Features)
- [x] **Intent Handling (Shared URLs)**:
    - [x] Update `AndroidManifest.xml` with `ACTION_SEND` intent filter.
    - [x] Create Kotlin Plugin (`IntentPlugin.kt`) to intercept `onNewIntent`. (Implemented in MainActivity.kt)
    - [x] Emit `intent://received` event to Tauri.
- [x] **Platform Notifications**:
    - [x] Integrate `tauri-plugin-notification`.
    - [ ] Wire Daemon "Notification" Protobuf messages to system notifications.

### Phase 4: Frontend & UI (Svelte 5)
- [x] **State Management**:
    - [x] Create `src/lib/stores/session.svelte.ts` using Runes for Tab/Session state.
- [x] **Terminal Component**:
    - [x] Build `Terminal.svelte` wrapping `xterm.js`.
    - [x] Implement `ResizeObserver` and `xterm-addon-fit`.
- [x] **Virtual Keyboard**:
    - [x] Port `KeySequenceHelper.kt` logic to TypeScript (`key-mapper.ts`).
    - [x] Build `VirtualKeyboard.svelte` (ModBar + AlphaGrid) with CSS transitions.
- [x] **Main Layout**:
    - [x] Implement SPA Tab View (Swipeable tabs for Terminal/Viewer).
- [x] **Viewer Component**:
    - [x] Create `Viewer.svelte` for Images/Markdown.

### Phase 5: Testing & Migration
- [ ] **Unit Tests**:
    - Rust: Test SSH config parsing and logic.
    - TS: Vitest for Key Mapper and Session Store.
- [ ] **E2E Tests**: Playwright tests for Tab switching.
- [ ] **Manual Verification**: Verify Intent handling on device.

## Task Log
- [ ] Created `PLAN.md`
