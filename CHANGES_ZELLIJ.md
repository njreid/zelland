# Implementation Plan - Zelland

## Overview
This document outlines the step-by-step implementation plan for building **Zelland** (Zellij + Android), a mobile terminal client with Zellij web integration, organized into clear milestones with measurable deliverables.

**Architecture**: SSH → Zellij Web → Port Forwarding → WebView

**What is Zelland?** A native Android app that connects to remote Zellij terminal multiplexer sessions via SSH, providing gesture-based navigation and multi-host session management.

---

## Milestone 1: Project Setup & SSH Integration
**Goal**: Establish the Android project structure and SSH connectivity using SSHJ.

### 1.1 Create Android Project
- [ ] Initialize new Android project with Kotlin support
- [ ] Set minSdk = 24, targetSdk = 34
- [ ] Configure Gradle build scripts (Kotlin DSL)
- [ ] Add required dependencies:
  - AndroidX Core KTX
  - AndroidX AppCompat
  - AndroidX Fragment KTX
  - Material Components
  - AndroidX ViewPager2
  - AndroidX Lifecycle (ViewModel, LiveData)
  - Kotlin Coroutines

### 1.2 Add SSH Dependencies
- [ ] Add SSHJ library: `com.hierynomus:sshj:0.39.0`
- [ ] Add BouncyCastle: `org.bouncycastle:bcprov-jdk18on:1.78`
- [ ] Add BouncyCastle PKIX: `org.bouncycastle:bcpkix-jdk18on:1.78`
- [ ] Add SLF4J Android: `org.slf4j:slf4j-android:1.7.36`
- [ ] Configure ProGuard rules for SSHJ (if using R8/ProGuard)

### 1.3 SSH Connection Manager
- [ ] Create `SSHConfig` data class (host, port, username, auth)
- [ ] Create `SSHConnectionManager` class:
  - `connect(config: SSHConfig)`
  - `executeCommand(command: String): String`
  - `disconnect()`
  - `isConnected(): Boolean`
- [ ] Implement password authentication
- [ ] Implement private key authentication
- [ ] Add connection timeout handling
- [ ] Add host key verification (PromiscuousVerifier for dev, proper verification for prod)

### 1.4 SSH Configuration UI
- [ ] Create `SSHConfigDialog` or `SSHConfigActivity`
- [ ] Add input fields:
  - Host (EditText)
  - Port (EditText, default 22)
  - Username (EditText)
  - Auth method (RadioGroup: Password / Private Key)
  - Password (EditText with password masking)
  - Private key path (File picker)
  - Key passphrase (EditText, optional)
- [ ] Add "Test Connection" button
- [ ] Add "Save" button
- [ ] Validate inputs before connecting

### 1.5 Basic Testing
- [ ] Test SSH connection to a known server
- [ ] Test executing simple commands (e.g., `whoami`, `pwd`)
- [ ] Test authentication failures
- [ ] Test network error handling

**Deliverable**: Can establish SSH connection, authenticate, and execute remote commands.

**Testing**: Connect to SSH server, run `echo "Hello from Android"`, verify output received.

---

## Milestone 2: Zellij Web Server Management
**Goal**: Start and manage Zellij web servers on remote hosts.

### 2.1 Zellij Startup Logic
- [ ] Add method `startZellijWeb(): Int` to SSHConnectionManager
- [ ] Check if Zellij is installed: `which zellij`
- [ ] Check if Zellij web is already running: `pgrep -f 'zellij web'`
- [ ] Start Zellij web in background: `nohup zellij web > /tmp/zellij-web.log 2>&1 &`
- [ ] Wait for server to start (delay + verification)
- [ ] Parse Zellij web port from process or config (default: 8082)
- [ ] Return port number

### 2.2 Zellij Session Management
- [ ] Add method `listZellijSessions(): List<String>`
- [ ] Execute: `zellij list-sessions`
- [ ] Parse session names from output
- [ ] Add method `attachZellijSession(sessionName: String)`
- [ ] Add method `killZellijSession(sessionName: String)`

### 2.3 Zellij Shutdown Logic
- [ ] Add method `stopZellijWeb()` to SSHConnectionManager
- [ ] Execute: `pkill -f 'zellij web'`
- [ ] Verify process terminated
- [ ] Handle case where Zellij is already stopped

### 2.4 Error Handling
- [ ] Handle Zellij not installed
- [ ] Handle Zellij web startup failure
- [ ] Handle permission errors
- [ ] Log Zellij output to Android logs

**Deliverable**: Can start/stop Zellij web server on remote host via SSH.

**Testing**: SSH to server, start Zellij web, verify process running with `pgrep`. Stop Zellij web, verify process killed.

---

## Milestone 3: SSH Port Forwarding
**Goal**: Tunnel Zellij web server to Android device via local port forwarding.

### 3.1 Port Forwarding Implementation
- [ ] Add method `setupPortForward(localPort: Int, remotePort: Int): PortForwardHandle`
- [ ] Use SSHJ's `LocalPortForwarder` API
- [ ] Create `PortForwardHandle` wrapper class:
  - `start()` - begins forwarding (runs in thread)
  - `close()` - stops forwarding
  - Properties: localPort, remotePort, isActive
- [ ] Handle port already in use errors

### 3.2 Port Management
- [ ] Add method `findAvailablePort(range: IntRange = 9000..9100): Int`
- [ ] Try binding ServerSocket to each port
- [ ] Return first available port
- [ ] Store active port forwards in map

### 3.3 Background Threading
- [ ] Run port forwarding in background thread/coroutine
- [ ] Use `Dispatchers.IO` for port forward listener
- [ ] Handle thread interruption gracefully
- [ ] Keep forwarding alive for session lifetime

### 3.4 Testing Port Forwarding
- [ ] Start Zellij web on remote host (port 8082)
- [ ] Set up port forward: localhost:9000 → remote:8082
- [ ] Test with `curl http://localhost:9000` from Android
- [ ] Verify Zellij web page loads

**Deliverable**: SSH tunnel successfully forwards remote Zellij web to local Android port.

**Testing**: Access `http://127.0.0.1:9000` from Android browser, see Zellij web interface.

---

## Milestone 4: WebView Integration with Zellij
**Goal**: Load Zellij web interface in Android WebView and handle sessions.

### 4.1 Basic WebView Setup
- [ ] Create `TerminalFragment` with WebView
- [ ] Configure WebView settings:
  - Enable JavaScript
  - Enable DOM storage
  - Allow mixed content (for localhost)
  - Disable zoom controls
- [ ] Set custom WebViewClient:
  - Handle page load events
  - Handle errors (connection refused, timeout)
  - Block external navigation

### 4.2 Load Zellij Web
- [ ] Add method `loadZellijWeb(url: String)` to Fragment
- [ ] Build URL: `http://127.0.0.1:{localPort}/{sessionName}`
- [ ] Load URL in WebView
- [ ] Show loading indicator
- [ ] Handle load success/failure

### 4.3 Session URL Management
- [ ] Update `TerminalSession` model:
  - Add `zellijSessionName` (persistent identifier)
  - Add `localUrl` (computed from port + session name)
  - Add `sshConfig`
  - Add `isConnected` flag
- [ ] Generate consistent session names (e.g., "project-alpha")
- [ ] Use same session name for reconnections (enables Zellij persistence)

### 4.4 ViewModel Integration
- [ ] Create `TerminalViewModel`:
  - `sessions: LiveData<List<TerminalSession>>`
  - `activeSessionIndex: LiveData<Int>`
  - `connectSession(sessionId: String, sshConfig: SSHConfig)`
  - `disconnectSession(sessionId: String)`
- [ ] Implement `connectSession()`:
  1. SSH connect
  2. Start Zellij web
  3. Set up port forward
  4. Generate local URL
  5. Update session state
- [ ] Implement `disconnectSession()`:
  - `softDisconnect()` - close SSH, keep Zellij running
  - `hardDisconnect()` - kill Zellij session

### 4.5 Connection Flow Testing
- [ ] Create session with SSH config
- [ ] Connect to session
- [ ] Verify Zellij web loads in WebView
- [ ] Type in terminal, verify it works
- [ ] Disconnect and reconnect, verify session persists

**Deliverable**: Full end-to-end flow: SSH → Zellij → Port Forward → WebView with working terminal.

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
- [ ] Create method `sendZellijTabSwitch(direction: SwipeDirection)`
- [ ] Generate JavaScript to simulate KeyboardEvent:
  - Set `altKey: true`
  - Set `key: "ArrowLeft"` or `"ArrowRight"`
  - Dispatch keydown and keyup events
- [ ] Execute JavaScript via `evaluateJavascript()`
- [ ] Test tab switching works in Zellij

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
**Goal**: Support multiple SSH hosts with independent Zellij sessions.

### 6.1 Session List Management
- [ ] Expand TerminalViewModel:
  - `addSession(sshConfig: SSHConfig)`
  - `removeSession(sessionId: String)`
  - `getSession(sessionId: String): TerminalSession?`
- [ ] Track SSHConnectionManager per session
- [ ] Store in map: `sessionId → SSHConnectionManager`

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
- [ ] Show SSHConfigDialog on FAB click
- [ ] Connect new session and add to ViewPager
- [ ] Add "Close Session" menu option
- [ ] Prompt: Soft disconnect or hard disconnect?
- [ ] Remove session from ViewPager on close

### 6.5 Session Indicators
- [ ] Add TabLayout or custom indicator above keyboard
- [ ] Show session names/hosts
- [ ] Highlight active session
- [ ] Update indicators on swipe

**Deliverable**: Multiple SSH connections with separate Zellij sessions, swipeable navigation.

**Testing**: Create 3 sessions to different hosts. Swipe between them. Verify each has independent terminal state. Close one session, verify others unaffected.

---

## Milestone 7: Session Persistence & Lifecycle
**Goal**: Persist sessions across app restarts and handle Android lifecycle properly.

### 7.1 Secure Storage
- [ ] Create `SecureSSHStorage` using Android Keystore
- [ ] Encrypt SSH passwords before storing
- [ ] Store SSH configs in EncryptedSharedPreferences
- [ ] Store session list (IDs, names, configs) in SharedPreferences
- [ ] Add methods:
  - `saveSSHConfig(config: SSHConfig)`
  - `loadSSHConfigs(): List<SSHConfig>`
  - `deleteSSHConfig(id: String)`

### 7.2 Session Persistence
- [ ] Save session list on `onPause()`
- [ ] Load session list on app startup
- [ ] Restore session metadata (not connections)
- [ ] Show "Reconnect" UI for saved sessions
- [ ] Handle corrupted storage gracefully

### 7.3 Lifecycle Management
- [ ] Implement `onPause()`:
  - Soft disconnect all sessions
  - Save session state
  - Keep Zellij sessions running on servers
- [ ] Implement `onResume()`:
  - Restore session list
  - Optionally auto-reconnect to last active session
- [ ] Implement `onDestroy()`:
  - Clean up SSH connections
  - Decide: soft disconnect (default) vs hard disconnect

### 7.4 Configuration Changes
- [ ] Handle device rotation
- [ ] Retain ViewModel across config changes
- [ ] Verify WebView state preserved
- [ ] Test rotation with active connections

### 7.5 Background Service (Optional)
- [ ] Create `SSHTunnelService` as ForegroundService
- [ ] Show persistent notification
- [ ] Keep SSH connections alive in background
- [ ] Allow reconnecting from notification

**Deliverable**: Sessions persist across app restarts. Zellij sessions continue running when app backgrounded.

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
  - "SSH connection failed"
  - "Zellij not installed on remote host"
  - "Port forwarding failed"
  - "Network unavailable"
- [ ] Add retry mechanism for transient failures
- [ ] Add "Check Logs" button in error dialog

### 8.3 Settings Screen
- [ ] Create SettingsActivity with PreferenceScreen
- [ ] Add preferences:
  - Auto-reconnect on startup (bool)
  - Gesture sensitivity (slider)
  - Edge swipe margin (int)
  - Default SSH port (int)
  - Zellij web port (int)
  - Keep sessions alive in background (bool)
- [ ] Apply settings in ViewModel

### 8.4 Performance Optimization
- [ ] Profile with Android Profiler
- [ ] Ensure 60 FPS swipe animation
- [ ] Minimize main thread blocking
- [ ] Test with 5+ simultaneous sessions
- [ ] Monitor memory usage (WebView can be heavy)

### 8.5 Logging & Debugging
- [ ] Add Timber for structured logging
- [ ] Log SSH connection events
- [ ] Log Zellij commands and responses
- [ ] Log port forwarding lifecycle
- [ ] Add debug mode in settings (verbose logs)

**Deliverable**: Polished app with good UX, error handling, and performance.

**Testing**: Test all error scenarios. Rotate device, verify stability. Open 5 sessions, check memory usage and smoothness.

---

## Milestone 9: Advanced Features (Optional)
**Goal**: Add power-user features and enhancements.

### 9.1 SSH Key Management
- [ ] Generate SSH keys on device
- [ ] Export public key for server deployment
- [ ] Store private keys securely in Keystore
- [ ] Support multiple key identities

### 9.2 Zellij Layout Management
- [ ] Send Zellij commands via JavaScript:
  - New tab (Ctrl+t)
  - Split pane (Ctrl+p, d)
  - Close pane (Ctrl+p, x)
  - Rename tab (Ctrl+r)
- [ ] Add toolbar with quick actions
- [ ] Support Zellij layouts (load/save)

### 9.3 Session Templates
- [ ] Save SSH configs as templates
- [ ] Quick connect to favorite hosts
- [ ] Group sessions by project/environment
- [ ] Import/export session configs

### 9.4 Network Monitoring
- [ ] Detect network changes (WiFi ↔ mobile)
- [ ] Auto-reconnect on network restore
- [ ] Show connection quality indicator
- [ ] Warn before using mobile data

### 9.5 External Keyboard Support
- [ ] Detect hardware keyboard
- [ ] Pass through keyboard events to WebView
- [ ] Support common terminal shortcuts
- [ ] Test with Bluetooth keyboard

**Deliverable**: Enhanced app with advanced features for power users.

---

## Milestone 10: Testing & Release Preparation
**Goal**: Ensure app stability and prepare for distribution.

### 10.1 Unit Testing
- [ ] Test `SSHConnectionManager` logic
- [ ] Test `TerminalViewModel` session management
- [ ] Test port allocation algorithm
- [ ] Test session persistence logic
- [ ] Target 80%+ coverage for business logic

### 10.2 Integration Testing
- [ ] Test SSH connection flow (requires test server)
- [ ] Test port forwarding (requires test server)
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
- [ ] Test various SSH servers (Linux, macOS)
- [ ] Test different network conditions
- [ ] Test with different Zellij versions

### 10.5 Documentation
- [ ] Update README.md:
  - App description
  - Features list
  - Screenshots
  - Build instructions
  - Usage guide
- [ ] Document SSH server requirements
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
| M1: Setup & SSH | 4-6 hours | None |
| M2: Zellij Management | 3-4 hours | M1 |
| M3: Port Forwarding | 3-4 hours | M1, M2 |
| M4: WebView Integration | 4-6 hours | M1, M2, M3 |
| M5: Gesture Controls | 4-5 hours | M4 |
| M6: Multi-Session | 4-6 hours | M4 |
| M7: Persistence | 5-7 hours | M6 |
| M8: Polish | 6-8 hours | M5, M6, M7 |
| M9: Advanced (optional) | 8-12 hours | M8 |
| M10: Testing & Release | 6-10 hours | All |
| **Total** | **47-68 hours** | |

## Critical Path

```
M1 (SSH) → M2 (Zellij) → M3 (Port Forward) → M4 (WebView) → M5 (Gestures)
                                                     ↓
                                                M6 (Multi-Session) → M7 (Persistence) → M8 (Polish) → M10 (Testing)
```

M5 can be developed in parallel with M6 after M4 is complete.

## Success Criteria

- ✅ Can connect to remote host via SSH with password or key
- ✅ Can start Zellij web server on remote host
- ✅ Can access Zellij web interface through port-forwarded WebView
- ✅ Can use terminal fully (type, navigate, run commands)
- ✅ Swipe gestures switch Zellij tabs
- ✅ Can manage multiple SSH/Zellij sessions
- ✅ Sessions persist across app restarts
- ✅ Zellij sessions continue running when Android app disconnects
- ✅ Smooth performance with 5+ sessions
- ✅ Handles network interruptions gracefully
- ✅ Works on Android 7.0+ devices

## Key Differences from Original Plan

### Removed Features (Not Needed with Zellij)
- ❌ Custom keyboard (ModBar/AlphaGrid) - Zellij web has built-in keyboard
- ❌ xterm.js local integration - Using Zellij's web interface instead
- ❌ JavaScript bridge for terminal I/O - WebView directly displays Zellij

### New Features (Zellij Integration)
- ✅ SSH connection management with SSHJ
- ✅ Remote Zellij web server lifecycle management
- ✅ SSH port forwarding for tunneling
- ✅ Gesture-to-keyboard-shortcut mapping
- ✅ Soft disconnect (keep Zellij running) vs hard disconnect
- ✅ Session resurrection (Zellij's built-in persistence)

### Architectural Benefits
- **Simpler Client**: No need to implement terminal emulation
- **Server-Side State**: Zellij handles all terminal state, buffers, and layout
- **Session Persistence**: Zellij automatically saves session state
- **Rich Features**: Tab management, panes, layouts all built into Zellij
- **Proven Technology**: Zellij web is battle-tested

### Trade-offs
- **Network Dependency**: Requires active SSH connection (no offline mode)
- **Server Requirement**: Must install Zellij on remote hosts
- **Additional Complexity**: SSH and port forwarding add setup steps
- **WebView Overhead**: Loading web interface in WebView uses more resources than native UI
