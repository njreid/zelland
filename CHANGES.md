# Implementation Plan - Zelland (Original xterm.js Version)

## Overview
This document outlines the original implementation plan for building Zelland as a standalone terminal app with xterm.js and custom keyboard. **Note**: This plan has been superseded by CHANGES_ZELLIJ.md which uses Zellij web integration instead.

**This document is kept for reference only.**

---

## Milestone 1: Project Setup & Foundation
**Goal**: Establish the Android project structure and basic WebView integration.

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

### 1.2 Setup xterm.js Assets
- [ ] Create `assets/terminal/` directory
- [ ] Download xterm.js v5.x and addons:
  - xterm.js core library
  - xterm.css
  - xterm-addon-fit.js
  - xterm-addon-web-links.js (optional)
- [ ] Create `index.html` with xterm.js integration
- [ ] Create `bridge.js` for Android ↔ JavaScript communication
- [ ] Test asset loading in WebView

### 1.3 Basic WebView Terminal
- [ ] Create `TerminalFragment` with WebView
- [ ] Configure WebView settings:
  - Enable JavaScript
  - Enable DOM storage
  - Disable zoom controls
  - Set text zoom to 100%
- [ ] Load local HTML from assets
- [ ] Verify xterm.js renders in WebView
- [ ] Test basic terminal input/output

**Deliverable**: Single terminal displaying in WebView with xterm.js rendering text.

**Testing**: Can type in terminal, see output, xterm.js loads correctly.

---

## Milestone 2: JavaScript Bridge & Input System
**Goal**: Establish bidirectional communication between Android and xterm.js.

### 2.1 JavaScript Interface
- [ ] Create `TerminalJavascriptInterface` class
- [ ] Add `@JavascriptInterface` methods:
  - `onTerminalReady()` - called when xterm.js initializes
  - `onTerminalData(data: String)` - called when user types in terminal
  - `onTerminalTitle(title: String)` - for session title updates
- [ ] Register interface with WebView
- [ ] Add JavaScript bridge code in `bridge.js`:
  - `term.onData()` handler
  - `Android.onTerminalData()` calls
  - `window.sendToTerminal()` function

### 2.2 Input Handling from Android
- [ ] Create `KeySequenceHelper` utility class:
  - Map keys to control sequences
  - Handle modifier combinations
  - Escape strings for JavaScript
- [ ] Implement `sendInput(key: String, modifiers: ModifierState)` in Fragment
- [ ] Use `evaluateJavascript()` to send to xterm.js
- [ ] Test control sequences (Ctrl+C, Ctrl+D, etc.)

### 2.3 Output Handling to Android
- [ ] Create callback mechanism in Fragment for terminal data
- [ ] Log terminal output for debugging
- [ ] Prepare for WebSocket forwarding (hook, don't implement)

**Deliverable**: Can send keys from Android code to xterm.js and receive terminal events.

**Testing**: Programmatically send "echo hello\n", see output in terminal. Send Ctrl+C, verify interrupt signal.

---

## Milestone 3: Custom Keyboard - ModBar
**Goal**: Build the persistent modifier key bar at the bottom of the screen.

### 3.1 ModBar Layout
- [ ] Create `view_mod_bar.xml`:
  - Horizontal LinearLayout
  - Buttons: Ctrl, Alt, Shift, Esc, Tab, ↑, ↓, ←, →
  - Fixed height: 48dp
  - Background: Material surface color
- [ ] Create `ModBarView` custom view class
- [ ] Define button styles and state colors
- [ ] Add ripple effects and haptic feedback

### 3.2 ModBar State Management
- [ ] Create `ModifierState` data class (ctrl, alt, shift flags)
- [ ] Implement toggle logic for Ctrl, Alt, Shift
- [ ] Implement direct action for Esc, Tab, arrows
- [ ] Add visual feedback for active modifiers:
  - Change background color
  - Update text color
  - Add border/stroke
- [ ] Expose state via LiveData or callback

### 3.3 ModBar Integration
- [ ] Add ModBar to `activity_main.xml`
- [ ] Connect ModBar to TerminalViewModel
- [ ] Test modifier toggling and visual states
- [ ] Test Esc, Tab, arrow key functionality

**Deliverable**: Persistent ModBar that can toggle modifiers and send special keys.

**Testing**: Tap Ctrl, see visual change. Tap Esc, verify escape sequence sent to terminal. Tap arrows, see cursor movement.

---

## Milestone 4: Custom Keyboard - AlphaGrid
**Goal**: Build the contextual alphanumeric keyboard that appears with modifiers.

### 4.1 AlphaGrid Layout
- [ ] Create `view_alpha_grid.xml`:
  - GridLayout or FlexboxLayout
  - 3 rows × 10 columns (30 keys)
  - Keys: A-Z, 0-9, common symbols (-, _, /, ., etc.)
  - Height: ~160dp
  - Initially GONE
- [ ] Create `AlphaGridView` custom view class
- [ ] Generate buttons programmatically or inflate from XML
- [ ] Style keys consistently with ModBar

### 4.2 AlphaGrid Animation
- [ ] Implement slide-up animation (ObjectAnimator)
- [ ] Implement slide-down animation
- [ ] Add animation duration: 250ms
- [ ] Use AccelerateDecelerateInterpolator
- [ ] Trigger animations based on modifier state

### 4.3 AlphaGrid Input Logic
- [ ] Add click listeners to all keys
- [ ] Combine key + active modifier state
- [ ] Generate appropriate control sequence
- [ ] Send to active terminal via TerminalViewModel
- [ ] Auto-dismiss after key press (optional setting)

### 4.4 CustomKeyboardView Container
- [ ] Create `view_custom_keyboard.xml`:
  - Vertical LinearLayout
  - AlphaGrid on top
  - ModBar on bottom
- [ ] Create `CustomKeyboardView` wrapper class
- [ ] Coordinate between AlphaGrid and ModBar
- [ ] Manage keyboard visibility state

**Deliverable**: Full custom keyboard with modifier bar and contextual alphanumeric grid.

**Testing**: Tap Ctrl, see AlphaGrid slide up. Tap C, verify Ctrl+C sent to terminal. Tap Ctrl again, see AlphaGrid slide down.

---

## Milestone 5: Multi-Session Support with ViewPager2
**Goal**: Enable multiple terminal sessions with swipe navigation.

### 5.1 TerminalViewModel
- [ ] Create `TerminalViewModel` class
- [ ] Add `sessions: MutableLiveData<List<TerminalSession>>`
- [ ] Add `activeSessionIndex: MutableLiveData<Int>`
- [ ] Add `modifierState: MutableLiveData<ModifierState>`
- [ ] Implement `addSession()`, `removeSession()`, `setActiveSession()`

### 5.2 TerminalSession Model
- [ ] Create `TerminalSession` data class:
  - `id: String` (UUID)
  - `title: String`
  - `webSocketUrl: String?`
  - `isConnected: Boolean`
  - `createdAt: Long`

### 5.3 ViewPager2 Setup
- [ ] Create `TerminalAdapter` extending `FragmentStateAdapter`
- [ ] Override `createFragment()` to return TerminalFragment
- [ ] Override `getItemCount()` based on sessions list
- [ ] Add ViewPager2 to `activity_main.xml`
- [ ] Connect adapter to ViewModel sessions

### 5.4 Session Lifecycle
- [ ] Ensure fragments are retained (not destroyed) on swipe
- [ ] Update activeSessionIndex on page change
- [ ] Route keyboard input to active session only
- [ ] Test with 3+ sessions

### 5.5 Session Management UI
- [ ] Add "+" FAB to create new session
- [ ] Add menu option to close current session
- [ ] Add TabLayout or page indicator for session list
- [ ] Update indicator on swipe

**Deliverable**: Multiple terminal sessions with swipe navigation and session management.

**Testing**: Create 3 sessions, swipe between them, verify each maintains separate terminal state. Type in session 1, swipe to session 2, swipe back, verify session 1 history preserved.

---

## Milestone 6: Polish & Optimization
**Goal**: Refine UI/UX, performance, and edge cases.

### 6.1 Theming & Styling
- [ ] Define color scheme in `colors.xml`:
  - Terminal background (dark)
  - Terminal foreground (light)
  - Keyboard background
  - Active modifier color
  - Ripple/press states
- [ ] Create dark theme (default)
- [ ] Optional: Create light theme
- [ ] Apply Material Design 3 guidelines
- [ ] Test on different screen sizes

### 6.2 Terminal Configuration
- [ ] Make font size configurable (Settings screen or menu)
- [ ] Make cursor style configurable (block/underline/bar)
- [ ] Make color scheme configurable (built-in themes)
- [ ] Save preferences to SharedPreferences
- [ ] Apply preferences on terminal initialization

### 6.3 Session Persistence
- [ ] Save session list to SharedPreferences on pause
- [ ] Restore session list on resume
- [ ] Serialize TerminalSession objects to JSON
- [ ] Handle session restoration gracefully (no WebSocket reconnect yet)

### 6.4 Performance Optimization
- [ ] Profile with Android Profiler
- [ ] Ensure 60 FPS swipe animation
- [ ] Minimize JavaScript bridge overhead
- [ ] Test memory usage with 5+ sessions
- [ ] Optimize WebView memory (consider process isolation)

### 6.5 Edge Case Handling
- [ ] Handle device rotation gracefully
- [ ] Prevent system keyboard from appearing
- [ ] Handle back button (close session? exit app?)
- [ ] Handle empty session list (show welcome screen)
- [ ] Handle WebView load errors
- [ ] Add proper error logging

**Deliverable**: Polished app with theming, persistence, and stable performance.

**Testing**: Rotate device, verify state preserved. Open 5 sessions, verify smooth performance. Close app, reopen, verify sessions restored.

---

## Milestone 7: Advanced Features (Optional)
**Goal**: Add enhancements for power users.

### 7.1 External Keyboard Support
- [ ] Detect hardware keyboard
- [ ] Intercept key events in Fragment
- [ ] Map to terminal sequences
- [ ] Test with Bluetooth keyboard

### 7.2 Gesture Shortcuts
- [ ] Volume Up/Down for special functions
- [ ] Long-press for context menu
- [ ] Two-finger swipe for quick session switch

### 7.3 Terminal Bells & Notifications
- [ ] Handle xterm.js bell event
- [ ] Show notification when app in background
- [ ] Add haptic feedback option

### 7.4 Copy/Paste Support
- [ ] Implement text selection in xterm.js
- [ ] Add copy button to toolbar
- [ ] Integrate with Android clipboard

### 7.5 Settings Screen
- [ ] Create SettingsActivity
- [ ] Add PreferenceScreen:
  - Font size
  - Theme
  - Cursor style
  - Haptic feedback
  - Bell notifications
  - Default WebSocket URL

**Deliverable**: Enhanced app with advanced features for improved UX.

---

## Milestone 8: Testing & Release Preparation
**Goal**: Ensure app stability and prepare for distribution.

### 8.1 Testing
- [ ] Unit tests for ViewModel logic
- [ ] Unit tests for KeySequenceHelper
- [ ] UI tests for keyboard input
- [ ] UI tests for session management
- [ ] Manual testing on multiple devices:
  - Phone (small screen)
  - Tablet (large screen)
  - Different Android versions (7.0, 10, 12, 14)

### 8.2 Documentation
- [ ] Update README.md with:
  - App description
  - Features list
  - Screenshots
  - Build instructions
  - Usage guide
- [ ] Add code comments for complex logic
- [ ] Document JavaScript bridge protocol

### 8.3 Release Build
- [ ] Configure ProGuard/R8 rules
- [ ] Test release build
- [ ] Set version code and name
- [ ] Generate signed APK/AAB
- [ ] Test on clean device (no debug tools)

### 8.4 Optional: Play Store Preparation
- [ ] Create app icon and assets
- [ ] Write store listing description
- [ ] Create screenshots and promo graphics
- [ ] Set up Play Console account
- [ ] Configure privacy policy

**Deliverable**: Production-ready app ready for distribution.

---

## Summary Timeline

| Milestone | Estimated Effort | Dependencies |
|-----------|------------------|--------------|
| M1: Setup | 2-4 hours | None |
| M2: Bridge | 3-5 hours | M1 |
| M3: ModBar | 3-4 hours | M2 |
| M4: AlphaGrid | 4-6 hours | M3 |
| M5: Multi-Session | 4-6 hours | M2 |
| M6: Polish | 4-6 hours | M3, M4, M5 |
| M7: Advanced | 6-10 hours | M6 (optional) |
| M8: Testing | 4-6 hours | All |
| **Total** | **30-47 hours** | |

## Critical Path

```
M1 (Setup) → M2 (Bridge) → M3 (ModBar) → M4 (AlphaGrid) → M6 (Polish)
                    ↓
                M5 (Multi-Session) → M6 (Polish)
```

M5 can be developed in parallel with M3/M4 after M2 is complete.

## Success Criteria

- ✅ Terminal displays and renders ANSI codes correctly
- ✅ Custom keyboard sends all standard terminal key sequences
- ✅ Can create and switch between multiple terminal sessions
- ✅ Sessions maintain state across swipes
- ✅ App performs smoothly (60 FPS) with 5+ sessions
- ✅ Sessions persist across app restarts
- ✅ No crashes on device rotation or lifecycle events
- ✅ Works on Android 7.0+ devices
