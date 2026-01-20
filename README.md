# Zelland

<div align="center">

**Zellij + Android = Zelland**

*A native Android client for Zellij terminal multiplexer*

[![Android](https://img.shields.io/badge/Platform-Android%207.0%2B-green.svg)](https://android.com)
[![Kotlin](https://img.shields.io/badge/Language-Kotlin-blue.svg)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/License-Apache%202.0-orange.svg)](LICENSE)

</div>

---

## What is Zelland?

**Zelland** is an Android application that brings the power of [Zellij](https://zellij.dev) terminal multiplexer to your mobile device. It connects to remote Zellij web servers via SSH, providing a native mobile interface with gesture controls and multi-host session management.

### Key Features

🔐 **SSH Integration**
- Connect to remote hosts with password or private key authentication
- Secure connections using SSHJ library
- Encrypted credential storage with Android Keystore

🌐 **Zellij Web Integration**
- Automatically starts and manages Zellij web servers
- SSH port forwarding tunnels Zellij to your device
- Full access to Zellij's terminal multiplexer features

📱 **Mobile-Optimized UI**
- Gesture controls for tab navigation (swipe left/right)
- ViewPager2 for switching between multiple SSH hosts
- Edge swipes vs center swipes for dual-layer navigation

🔄 **Session Persistence**
- Zellij sessions continue running even when disconnected
- Soft disconnect: Keep sessions alive on server
- Hard disconnect: Terminate sessions when done
- Auto-reconnect to existing sessions

🖥️ **Multi-Session Management**
- Connect to multiple remote hosts simultaneously
- Each host gets its own Zellij session
- Swipe between sessions seamlessly
- Session list with connection status indicators

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Zelland App                     │
│  ┌───────────────────────────────────────────┐  │
│  │          WebView (Zellij Web)             │  │
│  │     Loading: http://localhost:9999/       │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │       SSH Port Forward (SSHJ)             │  │
│  │    localhost:9999 → remote:8082           │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │          SSH Connection                    │  │
│  │    Connection to remote host               │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
                       ↓↑
┌─────────────────────────────────────────────────┐
│              Remote SSH Host                     │
│  ┌───────────────────────────────────────────┐  │
│  │      Zellij Web Server (port 8082)        │  │
│  │         Started via: zellij web           │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

**On your remote server:**
- SSH server running
- [Zellij](https://zellij.dev) installed (version with web support)

**On your Android device:**
- Android 7.0 (API 24) or higher
- Internet connection (WiFi or mobile data)

### Installation

1. Download the latest APK from [Releases](https://github.com/yourusername/zelland/releases)
2. Install on your Android device
3. Grant necessary permissions (Internet access)

### Usage

1. **Add SSH Connection**
   - Tap the "+" button
   - Enter host, username, and authentication credentials
   - Test connection

2. **Connect to Session**
   - Select a saved connection
   - Zelland will:
     - Establish SSH connection
     - Start Zellij web server (if not running)
     - Set up port forwarding
     - Load Zellij in WebView

3. **Navigate Sessions**
   - **Center swipe left/right**: Switch Zellij tabs (Alt+Arrow)
   - **Edge swipe**: Switch between different SSH hosts
   - **Long press**: Context menu (new tab, split pane, etc.)

4. **Manage Sessions**
   - Tap session name to reconnect
   - Swipe to close (soft disconnect)
   - Long press → Delete (hard disconnect)

## Building from Source

### Clone Repository

```bash
git clone https://github.com/yourusername/zelland.git
cd zelland
```

### Build

**Option A: Using Task (Recommended)**
```bash
# Install Task first: https://taskfile.dev
# macOS: brew install go-task
# Linux: sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b ~/.local/bin

# Build and run
task run

# Full development workflow with logs
task dev

# Push to remote emulator
task push -- hostname

# See all available commands
task
```

**Option B: Using Gradle**
```bash
./gradlew assembleDebug
```

See **[TASKFILE_USAGE.md](TASKFILE_USAGE.md)** for complete Task documentation.

### Run Tests

**Using Task:**
```bash
task test              # Unit tests
task lint              # Lint checks
task check             # All checks
```

**Using Gradle:**
```bash
# Unit tests
./gradlew test

# Instrumentation tests (requires device/emulator)
./gradlew connectedAndroidTest
```

### Install on Device

**Using Task:**
```bash
task install           # Local device
task push -- hostname  # Remote emulator
```

**Using Gradle:**
```bash
./gradlew installDebug
```

## Project Structure

```
zelland/
├── DESIGN.md              # Architecture and design decisions
├── CHANGES_ZELLIJ.md      # Implementation roadmap
├── SSH_INTEGRATION.md     # SSH and Zellij integration details
├── TESTING.md             # Testing strategy (in DESIGN.md)
├── app/
│   ├── src/main/
│   │   ├── java/com/zelland/
│   │   │   ├── MainActivity.kt
│   │   │   ├── ui/
│   │   │   │   └── terminal/
│   │   │   │       ├── TerminalFragment.kt
│   │   │   │       └── TerminalWebView.kt
│   │   │   ├── ssh/
│   │   │   │   └── SSHConnectionManager.kt
│   │   │   ├── viewmodel/
│   │   │   │   └── TerminalViewModel.kt
│   │   │   └── model/
│   │   │       └── TerminalSession.kt
│   │   ├── res/
│   │   └── AndroidManifest.xml
│   └── build.gradle.kts
└── build.gradle.kts
```

## Technology Stack

- **Language**: Kotlin
- **UI**: Android Views + ViewPager2 + WebView
- **Architecture**: MVVM (ViewModel + LiveData)
- **SSH Library**: [SSHJ](https://github.com/hierynomus/sshj)
- **Async**: Kotlin Coroutines
- **Security**: Android Keystore + EncryptedSharedPreferences
- **Testing**: JUnit, Espresso, MockK

## Roadmap

See [CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md) for detailed implementation milestones.

- [x] Project setup and architecture
- [ ] SSH connection management (M1)
- [ ] Zellij web server lifecycle (M2)
- [ ] SSH port forwarding (M3)
- [ ] WebView integration (M4)
- [ ] Gesture controls (M5)
- [ ] Multi-session support (M6)
- [ ] Session persistence (M7)
- [ ] Polish and optimization (M8)
- [ ] Advanced features (M9)
- [ ] Testing and release (M10)

## Documentation

- **[DESIGN.md](DESIGN.md)** - Complete architecture and design document
- **[CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md)** - Step-by-step implementation plan
- **[SSH_INTEGRATION.md](SSH_INTEGRATION.md)** - SSH and Zellij integration guide
- **[BUILD.md](BUILD.md)** - Build system and troubleshooting
- **[TASKFILE_USAGE.md](TASKFILE_USAGE.md)** - Task automation guide
- **[QUICKSTART.md](QUICKSTART.md)** - 5-minute setup guide

## Contributing

Contributions are welcome! Please read our [Contributing Guidelines](CONTRIBUTING.md) first.

### Development Setup

1. Install [Android Studio](https://developer.android.com/studio)
2. Clone the repository
3. Open project in Android Studio
4. Set up a test SSH server (or use your own)
5. Run on device/emulator

### Code Style

- Follow [Kotlin coding conventions](https://kotlinlang.org/docs/coding-conventions.html)
- Use `ktlint` for formatting
- Write tests for new features

## FAQ

### Why Zelland instead of Termux?

Zelland is not a replacement for Termux. It's designed for:
- **Remote sessions**: Connect to existing servers
- **Zellij integration**: Leverage Zellij's powerful multiplexer features
- **Mobile UX**: Gesture-based navigation optimized for touch

Use Termux if you need a local terminal emulator on Android.

### Does Zelland work offline?

No. Zelland requires an active network connection to maintain SSH tunnels to remote Zellij servers. However, Zellij sessions persist on the server when you disconnect, so you can reconnect later.

### What versions of Zellij are supported?

Zelland requires Zellij 0.43.0+ (when web client support was added).

### Can I use Zelland with tmux/screen instead of Zellij?

Currently, Zelland is designed specifically for Zellij's web interface. Support for other multiplexers would require significant architectural changes.

### How secure is Zelland?

- SSH connections use industry-standard encryption
- Credentials stored in Android Keystore
- Port forwarding is local-only (127.0.0.1)
- No data sent to third parties
- All traffic encrypted via SSH tunnel

Always use strong passwords/keys and keep your devices updated.

## License

```
Copyright 2026 Zelland Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

## Acknowledgments

- [Zellij](https://zellij.dev) - The terminal multiplexer that powers Zelland
- [SSHJ](https://github.com/hierynomus/sshj) - SSH library for Java/Android
- [xterm.js](https://xtermjs.org) - Terminal emulator used by Zellij web

## Contact

- GitHub Issues: [Report bugs or request features](https://github.com/yourusername/zelland/issues)
- Discussions: [Ask questions and share ideas](https://github.com/yourusername/zelland/discussions)

---

<div align="center">
Made with ❤️ for the terminal-loving mobile community
</div>
