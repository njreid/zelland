# Implementation Plan - Zelland

## Overview

This document outlines the step-by-step implementation plan for building **Zelland** (Zellij + Android), a mobile terminal client with Zellij web integration, organized into clear milestones with measurable deliverables.

**Architecture**: Direct HTTPS Connection to Zellij Web Server + WebSocket Control Channel to Companion Daemon (`zellandd`)

---

## Milestone 4: WebView Integration with Zellij
*Status: Complete*

- [x] Create `TerminalFragment` (migrated to Compose `TerminalScreen`)
- [x] Configure WebView settings for Zellij
- [x] Handle page load and SSL errors
- [x] Build and load dynamic session URLs

---

## Milestone 5: Gesture Controls for Zellij Navigation
*Status: Complete*

- [x] Create `TerminalWebView` with custom gesture detection
- [x] Detect horizontal swipes for tab switching
- [x] Inject Alt+Left/Right keys via native Android events
- [x] Coordinate with system keyboard and modifier bar

---

## Milestone 6: Multi-Session Support
*Status: In Progress*

- [ ] Expand `TerminalViewModel` to manage multiple active connections
- [ ] Implement `Pager` in Compose to swipe between sessions
- [ ] Add session indicators/tabs
- [ ] Implement robust "Add Session" and "Delete Session" flow

---

## Milestone 11: Companion Daemon Integration (Foundation)
*Status: Complete*

- [x] Add `com.google.protobuf:protobuf-kotlin-lite` and relevant plugins to `build.gradle.kts`
- [x] Import `zelland.proto` definition into the project
- [x] Generate Kotlin Protobuf classes
- [x] Implement `DaemonConnectionManager` using OkHttp WebSockets
- [x] Handle `wss://` protocol and trust managers (matching terminal SSL logic)
- [x] Implement PSK (Pre-Shared Key) authentication in handshake headers
- [x] Implement Heartbeat mechanism: Respond to `KeepAlive` (Ping) messages
- [x] Update `TerminalViewModel` to track Daemon connectivity status
- [x] Implement auto-reconnect logic on app foreground or network restore
- [x] Add connection status indicators to the UI

---

## Milestone 12: Universal Viewer Implementation
*Status: Complete*

- [x] Implement handler for `OpenViewRequest` Protobuf message
- [x] Create `ViewerTab` composable to render non-terminal content (migrated to `ViewerScreen`)
- [x] Extend navigation logic to switch from Terminal to Viewer when a message arrives
- [x] Implement `ImageViewer` with zoom/pan support (via Coil + transformable state)
- [x] Integrate PDF rendering library or specialized WebView for PDFs (using `GenericWebViewer`)
- [x] Handle ephemeral asset downloading via `https://<host>:8083/assets/<id>`
- [x] Send `ClientStatus` updates when a viewer tab is opened/closed
- [x] Implement "Close Viewer" action to return to the terminal

---

## Milestone 13: Markdown & Annotation System
**Goal**: Enable rich Markdown viewing with bidirectional annotation syncing.

### 13.1 Markdown Renderer
- [ ] Implement specialized WebView or native renderer for Markdown content
- [ ] Support text selection and context menu integration

### 13.2 Annotation Syncing
- [ ] Implement `MarkdownSessionStart` handler to load existing annotations
- [ ] Implement `AnnotationAction` Protobuf emitter for creating/updating notes
- [ ] Visualizing annotations (highlights) in the Markdown view
- [ ] Implement context-aware anchoring using paragraph hashes (SHA256)

---

## Milestone 14: Background Connectivity & FCM
**Goal**: Ensure reliability when the app is backgrounded.

### 14.1 FCM Wake-up
- [ ] Integrate Firebase Cloud Messaging (FCM) SDK
- [ ] Implement `ZellandMessagingService` to handle high-priority data messages
- [ ] Trigger WebSocket reconnection when an FCM "wake-up" is received

### 14.2 Network Optimization
- [ ] Implement battery-optimized WebSocket settings (adjusted ping intervals)
- [ ] Handle network transitions (WiFi ↔ Mobile) gracefully without dropping state
