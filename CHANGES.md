# Implementation Plan - Zelland

## Overview

This document outlines the step-by-step implementation plan for building **Zelland** (Zellij + Android), a mobile terminal client with Zellij web integration, organized into clear milestones with measurable deliverables.

**Architecture**: Direct HTTPS Connection to Zellij Web Server

**What is Zelland?** A native Android app that connects to remote Zellij terminal multiplexer sessions via HTTPS, providing gesture-based navigation and multi-host session management.

---

## Milestone 4: WebView Integration with Zellij

**Goal**: Load Zellij web interface in Android WebView and handle sessions.

### 4.1 Basic WebView Setup

- [x] Create `TerminalFragment` with WebView
- [x] Configure WebView settings:
  - Enable JavaScript
  - Enable DOM storage
  - Allow mixed content (for localhost)
  - Disable zoom controls
- [x] Set custom WebViewClient:
  - Handle page load events
  - Handle errors (connection refused, timeout)
  - Block external navigation

### 4.2 Load Zellij Web

- [x] Add method `loadZellijWeb(url: String)` to Fragment
- [x] Build URL: `https://{host}:{port}/{sessionName}`
- [x] Load URL in WebView
- [x] Show loading indicator
- [x] Handle load success/failure

### 4.3 Session URL Management

- [x] Update `TerminalSession` model:
  - Add `zellijSessionName` (persistent identifier)
  - Add `localUrl` (computed from port + session name)
  - Add `sshConfig` (now just host/name config)
  - Add `isConnected` flag
- [x] Generate consistent session names (e.g., "project-alpha")
- [x] Use same session name for reconnections (enables Zellij persistence)

### 4.4 ViewModel Integration

- [x] Create `TerminalViewModel`:
  - `sessions: LiveData<List<TerminalSession>>`
  - `activeSessionIndex: LiveData<Int>`
  - `connectSession(sessionId: String)`
  - `disconnectSession(sessionId: String)`
- [x] Implement `connectSession()`:
  1. Check direct HTTPS connection
  2. Update session state
- [x] Implement `disconnectSession()`:
  - Update session state to disconnected

### 4.5 Connection Flow Testing

- [x] Create session with host config
- [x] Connect to session
- [x] Verify Zellij web loads in WebView
- [x] Type in terminal, verify it works
- [x] Disconnect and reconnect, verify session persists

**Deliverable**: Full end-to-end flow: HTTPS → WebView with working terminal.

**Testing**: Create session, connect, type commands in terminal, see output. Disconnect, reconnect, verify session state preserved.

---

## Milestone 5: Gesture Controls for Zellij Navigation

**Goal**: Map Android gestures to Zellij keyboard shortcuts.

### 5.1 Custom TerminalWebView

- [ ] Create `TerminalWebView` extending WebView
- [ ] Add GestureDetector for swipe detection
- [ ] Configure swipe parameters:
  - MIN_SWIPE_DISTANCE (100px)
  - MIN_SWIPE_VELOCITY (100px/sec)
  - EDGE_SWIPE_MARGIN (50px)
- [ ] Detect horizontal swipes
- [ ] Distinguish edge swipes from center swipes

### 5.2 Swipe-to-Tab-Switch

- [ ] Implement `onFling()` handler
- [ ] Detect swipe direction (left/right)
- [ ] Ignore edge swipes (for ViewPager2)
- [ ] Send Alt+Arrow to Zellij for center swipes
- [ ] Add callback: `setOnZellijTabSwipeListener()`

### 5.3 JavaScript Key Injection

- [x] Create method `sendZellijTabSwitch(direction: SwipeDirection)`
- [x] Generate JavaScript to simulate KeyboardEvent:
  - Set `altKey: true`
  - Set `key: "ArrowLeft"` or `"ArrowRight"`
  - Dispatch keydown and keyup events
- [x] Execute JavaScript via `evaluateJavascript()`
- [x] Test tab switching works in Zellij

### 5.4 ViewPager2 Coordination

- [ ] Configure ViewPager2 to allow edge swipes only
- [ ] Reduce ViewPager2 sensitivity
- [ ] Add touch interceptor to prioritize terminal swipes
- [ ] Test both navigation layers work correctly

### 5.5 Additional Gestures (Optional)

- [ ] Long press → Context menu (new tab, split pane, etc.)
- [ ] Three-finger swipe → Special functions
- [ ] Volume keys → Page up/down

**Deliverable**: Swipe left/right on terminal switches Zellij tabs. Edge swipes switch Android sessions.

**Testing**: Create Zellij session with multiple tabs. Swipe center-left to go to previous tab. Swipe center-right to go to next tab. Swipe from left edge to switch Android session.

---

## Milestone 6: Multi-Session Support

**Goal**: Support multiple hosts with independent Zellij sessions.

### 6.1 Session List Management

- [ ] Expand TerminalViewModel:
  - `addSession(config: SSHConfig)`
  - `removeSession(sessionId: String)`
  - `getSession(sessionId: String): TerminalSession?`

### 6.2 ViewPager2 Setup

- [ ] Create `TerminalAdapter` extending FragmentStateAdapter
- [ ] Override `createFragment()` to return TerminalFragment
- [ ] Override `getItemCount()` from sessions list
- [ ] Pass session ID to each fragment via Bundle
- [ ] Add ViewPager2 to MainActivity layout

### 6.3 Session Switching

- [ ] Add ViewPager2.OnPageChangeCallback
- [ ] Update activeSessionIndex on page change
- [ ] Ensure only active session receives input
- [ ] Test with 3+ sessions

### 6.4 Add/Remove Sessions UI

- [ ] Add FAB (Floating Action Button) for new session
- [ ] Show ConfigDialog on FAB click
- [ ] Connect new session and add to ViewPager
- [ ] Add "Close Session" menu option
- [ ] Remove session from ViewPager on close

### 6.5 Session Indicators

- [ ] Add TabLayout or custom indicator above keyboard
- [ ] Show session names/hosts
- [ ] Highlight active session
- [ ] Update indicators on swipe

**Deliverable**: Multiple connections with separate Zellij sessions, swipeable navigation.

**Testing**: Create 3 sessions to different hosts. Swipe between them. Verify each has independent terminal state. Close one session, verify others unaffected.

---

## Milestone 7: Session Persistence & Lifecycle

**Goal**: Persist sessions across app restarts and handle Android lifecycle properly.

### 7.1 Secure Storage

- [ ] Store session list (IDs, names, configs) in SharedPreferences
- [ ] Add methods:
  - `saveConfig(config: SSHConfig)`
  - `loadConfigs(): List<SSHConfig>`
  - `deleteConfig(id: String)`

### 7.2 Session Persistence

- [ ] Save session list on `onPause()`
- [ ] Load session list on app startup
- [ ] Restore session metadata (not connections)
- [ ] Show "Reconnect" UI for saved sessions
- [ ] Handle corrupted storage gracefully

### 7.3 Lifecycle Management

- [ ] Implement `onPause()`:
  - Disconnect all sessions
  - Save session state
- [ ] Implement `onResume()`:
  - Restore session list
  - Optionally auto-reconnect to last active session
- [ ] Implement `onDestroy()`:
  - Clean up connections

### 7.4 Configuration Changes

- [ ] Handle device rotation
- [ ] Retain ViewModel across config changes
- [ ] Verify WebView state preserved
- [ ] Test rotation with active connections

**Deliverable**: Sessions persist across app restarts.

**Testing**: Create session, run long process (e.g., `sleep 300`). Close app. Reopen app, reconnect to session, verify process still running.

---

## Milestone 8: Polish & Optimization

**Goal**: Refine UI/UX, performance, and edge cases.

### 8.1 Theming & Styling

- [ ] Define color scheme (dark theme default)
- [ ] Style WebView background to match terminal theme
- [ ] Style session indicators
- [ ] Add Material Design 3 components
- [ ] Test on different screen sizes (phone, tablet)

### 8.2 Error Handling & UX

- [ ] Show connection progress dialog
- [ ] Display friendly error messages:
  - "Connection failed"
  - "Network unavailable"
- [ ] Add retry mechanism for transient failures

### 8.3 Settings Screen

- [ ] Create SettingsActivity with PreferenceScreen
- [ ] Add preferences:
  - Auto-reconnect on startup (bool)
  - Gesture sensitivity (slider)
  - Edge swipe margin (int)
  - Zellij web port (int)
- [ ] Apply settings in ViewModel

### 8.4 Performance Optimization

- [ ] Profile with Android Profiler
- [ ] Ensure 60 FPS swipe animation
- [ ] Minimize main thread blocking
- [ ] Test with 5+ simultaneous sessions
- [ ] Monitor memory usage (WebView can be heavy)

### 8.5 Logging & Debugging

- [ ] Add Timber for structured logging
- [ ] Log connection events
- [ ] Add debug mode in settings (verbose logs)

**Deliverable**: Polished app with good UX, error handling, and performance.

**Testing**: Test all error scenarios. Rotate device, verify stability. Open 5 sessions, check memory usage and smoothness.

---

## Milestone 9: Advanced Features (Optional)

**Goal**: Add power-user features and enhancements.

### 9.1 Zellij Layout Management

- [ ] Send Zellij commands via JavaScript:
  - New tab (Ctrl+t)
  - Split pane (Ctrl+p, d)
  - Close pane (Ctrl+p, x)
  - Rename tab (Ctrl+r)
- [ ] Add toolbar with quick actions
- [ ] Support Zellij layouts (load/save)

### 9.2 Session Templates

- [ ] Save configs as templates
- [ ] Quick connect to favorite hosts
- [ ] Group sessions by project/environment
- [ ] Import/export session configs

### 9.3 Network Monitoring

- [ ] Detect network changes (WiFi ↔ mobile)
- [ ] Auto-reconnect on network restore
- [ ] Show connection quality indicator
- [ ] Warn before using mobile data

### 9.4 External Keyboard Support

- [ ] Detect hardware keyboard
- [ ] Pass through keyboard events to WebView
- [ ] Support common terminal shortcuts
- [ ] Test with Bluetooth keyboard

**Deliverable**: Enhanced app with advanced features for power users.

---

## Milestone 10: Testing & Release Preparation

**Goal**: Ensure app stability and prepare for distribution.

### 10.1 Unit Testing

- [ ] Test `TerminalViewModel` session management
- [ ] Test session persistence logic
- [ ] Target 80%+ coverage for business logic

### 10.2 Integration Testing

- [ ] Test connection flow (requires test server)
- [ ] Test WebView loading Zellij web
- [ ] Test gesture detection
- [ ] Test multi-session management

### 10.3 UI Testing (Espresso)

- [ ] Test session creation flow
- [ ] Test connection dialog
- [ ] Test ViewPager2 swiping
- [ ] Test session disconnect
- [ ] Test settings screen

### 10.4 Manual Testing

- [ ] Test on multiple devices:
  - Phone (Android 7.0, 10, 12, 14)
  - Tablet
  - Foldable (if available)
- [ ] Test different network conditions
- [ ] Test with different Zellij versions

### 10.5 Documentation

- [ ] Update README.md:
  - App description
  - Features list
  - Screenshots
  - Build instructions
  - Usage guide
- [ ] Document server requirements
- [ ] Document Zellij installation on server
- [ ] Add troubleshooting guide

### 10.6 Release Build

- [ ] Configure ProGuard/R8 rules
- [ ] Test release build thoroughly
- [ ] Set version code and name
- [ ] Generate signed APK/AAB
- [ ] Test on clean device

### 10.7 Play Store Preparation (Optional)

- [ ] Create app icon and launcher assets
- [ ] Write store listing description
- [ ] Create screenshots (phone + tablet)
- [ ] Create feature graphic
- [ ] Set up Play Console account
- [ ] Write privacy policy

**Deliverable**: Production-ready app ready for distribution.

---

## Summary Timeline

| Milestone | Estimated Effort | Dependencies |
|-----------|------------------|--------------|
| M4: WebView Integration | 4-6 hours | None |
| M5: Gesture Controls | 4-5 hours | M4 |
| M6: Multi-Session | 4-6 hours | M4 |
| M7: Persistence | 5-7 hours | M6 |
| M8: Polish | 6-8 hours | M5, M6, M7 |
| M9: Advanced (optional) | 8-12 hours | M8 |
| M10: Testing & Release | 6-10 hours | All |
| **Total** | **37-54 hours** | |

## Critical Path

```text
M4 (WebView) → M5 (Gestures)
      ↓
M6 (Multi-Session) → M7 (Persistence) → M8 (Polish) → M10 (Testing)
```

M5 can be developed in parallel with M6 after M4 is complete.

## Success Criteria

- ✅ Can connect to remote host via HTTPS
- ✅ Can access Zellij web interface through WebView
- ✅ Can use terminal fully (type, navigate, run commands)
- ✅ Swipe gestures switch Zellij tabs
- ✅ Can manage multiple sessions
- ✅ Sessions persist across app restarts
- ✅ Smooth performance with 5+ sessions
- ✅ Handles network interruptions gracefully
- ✅ Works on Android 7.0+ devices

## Key Differences from Original Plan

### Removed Features (Not Needed with Zellij)

- ❌ Custom keyboard (ModBar/AlphaGrid) - Zellij web has built-in keyboard
- ❌ xterm.js local integration - Using Zellij's web interface instead
- ❌ JavaScript bridge for terminal I/O - WebView directly displays Zellij
- ❌ SSH Tunneling - Direct HTTPS connection used instead

### New Features (Zellij Integration)

- ✅ Direct HTTPS connection management
- ✅ Gesture-to-keyboard-shortcut mapping
- ✅ Session resurrection (Zellij's built-in persistence)

### Architectural Benefits

- **Simpler Client**: No need to implement terminal emulation or SSH tunneling
- **Server-Side State**: Zellij handles all terminal state, buffers, and layout
- **Session Persistence**: Zellij automatically saves session state
- **Rich Features**: Tab management, panes, layouts all built into Zellij
- **Proven Technology**: Zellij web is battle-tested

### Trade-offs

- **Network Dependency**: Requires active connection (no offline mode)
- **Server Requirement**: Must install Zellij on remote hosts and expose web port
- **WebView Overhead**: Loading web interface in WebView uses more resources than native UI
