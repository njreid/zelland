# Zelland

**Zellij + Android = Zelland**

*A native Android client for Zellij terminal multiplexer*

[![Android](https://img.shields.io/badge/Platform-Android%207.0%2B-green.svg)](https://android.com)
[![Kotlin](https://img.shields.io/badge/Language-Kotlin-blue.svg)](https://kotlinlang.org)
[![Compose](https://img.shields.io/badge/UI-Jetpack%20Compose-orange.svg)](https://developer.android.com/jetpack/compose)

---

## What is Zelland?

**Zelland** is a modern Android application that brings the power of [Zellij](https://zellij.dev) terminal multiplexer to your mobile device. Built with **Jetpack Compose**, it provides a streamlined interface for connecting directly to remote Zellij web servers via HTTPS, featuring specialized terminal controls and multi-session management.

{}

### Key Features

🌐 **Direct HTTPS Integration**

- Connect directly to remote Zellij web servers (default port 8082).
- Support for session-specific routes (e.g., `https://host/my-session`).
- Automatic trust for self-signed certificates (ideal for private networks like Tailscale).

⌨️ **Specialized Terminal Shortcut Bar**

- **Persistent Modifiers**: **C** (Ctrl), **A** (Alt), and **M** (Meta) with smart latching:
o

- *Single Tap*: One-shot modifier for the next key.
- *Double Tap*: Locked state for repetitive shortcuts.
- **Gesture-Based Navigation**:
  - **4-Way Arrow Button**: Drag in any direction to send a single arrow key.
  - **Cardinal Overlay**: Tap to reveal a multi-click D-Pad with PgUp/PgDn for rapid navigation.
- **Optimized Keys**: Dedicated **ESC** (compact), **Tab** (`|->`), and **Enter** buttons.

📱 **Mobile-First UX**

- **Jetpack Compose UI**: Smooth, reactive interface with dark terminal aesthetics.
- **Slim Keyboard**: System keyboard is optimized (`textNoSuggestions`) to maximize screen space.
- **Session Persistence**: Sessions are stored locally and reloaded automatically on app start.

🔤 **Terminal Rendering**

- Forced monospace font rendering.
- Disabled ligatures and optimized letter spacing for perfect character alignment.

## Architecture

```text
┌─────────────────────────────────────────────────┐
│                  Zelland App                     │
│  ┌───────────────────────────────────────────┐  │
│  │          Jetpack Compose UI               │  │
│  │      (Terminal Bar + Modifiers)           │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │          Android WebView                  │  │
│  │     Loading: https://host:8082/session    │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
                       ↓↑
┌─────────────────────────────────────────────────┐
│              Remote Host (Zellij)                │
│  ┌───────────────────────────────────────────┐  │
│  │      Zellij Web Server (port 8082)        │  │
│  │         Started via: zellij web           │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

**On your remote server:**

- [Zellij](https://zellij.dev) installed (0.43.0+ for web support).
- Zellij web server running: `zellij web`.

**On your Android device:**

- Android 7.0 (API 24) or higher.
- Network access to the remote host (e.g., via Tailscale or VPN).

### Usage

1. **Add Session**
   - Tap the **"+"** button on the main screen.
   - Enter a **Connection Name**, the **Host** address, and an optional **Zellij Session Name**.
   - If a session name is provided, the app will connect to that specific route.

2. **Connect**
   - Tap **"Connect"** on a session card.
   - Zelland will verify reachability and load the terminal in the native WebView.

3. **Navigate**
   - Use the **Shortcut Bar** for modifiers and special keys.
   - **Double-tap** C/A/M to lock them on.
   - **Drag** the arrows button for single movements or **tap** it for the cardinal overlay.
   - **Long-press** PgDn in the overlay to send PgUp.

## Building from Source

### Prerequisites

- Android Studio Iguana or newer.
- Kotlin 1.9.22+.

### Build

```bash
./gradlew assembleDebug
```

## Project Structure

```text
zelland/
├── app/src/main/java/com/zelland/
│   ├── MainActivity.kt        # Compose Entry point
│   ├── ui/
│   │   ├── MainScreen.kt      # Session list UI
│   │   ├── TerminalScreen.kt  # Terminal UI & specialized bar
│   │   ├── TerminalWebView.kt # Modified WebView for terminal input
│   │   └── ZellandApp.kt      # Main Navigation logic
│   ├── viewmodel/
│   │   └── TerminalViewModel.kt # Session management logic
│   └── model/
│       └── TerminalSession.kt # Data model
└── build.gradle.kts           # Build configuration
```

## License

```text
Copyright 2026 Zelland Contributors
Licensed under the Apache License, Version 2.0
```

---

Made with ❤️ for the terminal-loving mobile community
