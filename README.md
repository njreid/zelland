# zelland

**A native Android + Linux terminal client built on Tauri, wgpu, and SSH**

[![Platform](https://img.shields.io/badge/Platform-Android%207.0%2B%20%7C%20Linux-green.svg)](https://android.com)
[![Rust](https://img.shields.io/badge/Language-Rust%20%2B%20Kotlin-blue.svg)](https://rust-lang.org)
[![Tauri](https://img.shields.io/badge/Framework-Tauri%20v2-orange.svg)](https://tauri.app)

---

## What is zelland?

zelland is a mobile-first SSH terminal and command center. It connects directly to remote hosts via SSH, renders the terminal natively using **wgpu + glyphon** (Vulkan on Android), and optionally syncs collaborative markdown annotations through a local Rust daemon (`zellandd`).

The app is built with **Tauri v2 + Svelte 5** for the UI layer and a hand-rolled Rust renderer for the terminal surface — no WebView terminal emulation.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Android Activity (MainActivity.kt)                           │
│                                                               │
│  DrawerLayout                                                 │
│  ├── FrameLayout (main content)                               │
│  │   ├── WebView  (Svelte app — welcome screen, modals)       │
│  │   └── SurfaceView  (wgpu Vulkan terminal surface)          │
│  └── LinearLayout (left sidebar — sessions + hosts tree)      │
│                                                               │
│  GestureDetector: tap → focus, 2-finger scroll, pinch zoom   │
│  KeybarPlugin: IME toolbar with Ctrl / Alt / Meta modifiers   │
└──────────────────────────────────────────────────────────────┘
         │ JNI                              │ JS bridge
         ▼                                  ▼
┌─────────────────┐              ┌──────────────────────┐
│  Rust (src-tauri)│             │  Svelte 5 frontend    │
│  wgpu renderer  │              │  welcome / modals     │
│  SSH manager    │              │  markdown pane        │
│  libghostty-vt  │              │  annotation editor    │
└────────┬────────┘              └──────────────────────┘
         │
         ▼
┌──────────────────────────────────────────┐
│  russh → SSH channel → remote host       │
│  VT bytes → libghostty-vt → render state │
│  Touch → SGR mouse sequences → SSH       │
└──────────────────────────────────────────┘
         │ HTTP REST + WebSocket
         ▼
┌──────────────────────────────────────────┐
│  zellandd (daemon-rs)                    │
│  axum + tokio · project/asset/annotation │
│  YJS-based collaborative sync            │
└──────────────────────────────────────────┘
```

---

## Key Features

### Terminal Rendering
- **wgpu + glyphon** rendering via Vulkan (Android) or system GPU (Linux)
- Full **ANSI color** support: 16-color palette + 24-bit RGB
- **Bold**, *italic*, reverse-video, and underline attributes
- Hardware-accelerated cursor rectangle
- **Text selection** with native Copy/Paste action bar
- **Pinch-to-zoom** font size scaling

### SSH & Connectivity
- Direct SSH with password or public-key authentication (`russh`)
- **SSH keepalives** (30 s interval) to survive background screen-off
- **WireGuard** tunnel support for private network access
- Per-session resize, scroll, and SGR mouse tracking

### Android Native UI
- **DrawerLayout sidebar**: swipe-left (or fling) opens a native sessions + hosts panel
- **Foreground service + wake lock**: SSH sessions survive screen lock
- **KeybarPlugin**: persistent IME toolbar with latching Ctrl/Alt/Meta modifiers and arrow keys
- Bottom-sheet modals (Add Host, Add Session, Settings) rendered through the WebView layer
- Back button closes open drawer; surface lifecycle handles screen lock/unlock

### Collaborative Annotations (`zellandd`)
- Local Rust daemon serving projects, assets, and markdown files
- **YJS-based real-time sync** for inline text and code-block annotations
- REST + WebSocket API consumed by the Svelte markdown pane

---

## Project Structure

```
zelland/
├── src/                        # Svelte 5 frontend
│   ├── routes/+page.svelte     # Main app shell
│   └── lib/
│       ├── components/         # Sidebar, MarkdownPane, modals
│       └── utils/              # key-mapper, time-ago, kb-input
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # Tauri commands
│   │   ├── ssh.rs              # SshManager, russh client
│   │   ├── terminal.rs         # TerminalSession (libghostty-vt)
│   │   ├── ghostty.rs          # C FFI bindings
│   │   ├── renderer/           # wgpu + glyphon renderer
│   │   │   ├── mod.rs          # draw_ghostty_state, render loop
│   │   │   └── android.rs      # JNI surface/touch/resize entry points
│   │   ├── network.rs          # WireGuard config
│   │   └── keystore.rs         # SSH key management
│   └── gen/android/            # Android project
│       └── app/src/main/java/com/njr/zelland/
│           ├── MainActivity.kt              # DrawerLayout, SurfaceView, gestures
│           ├── KeybarPlugin.kt             # IME toolbar
│           ├── KeySeqs.kt                  # IME char → terminal sequence mapping
│           ├── KeybarSeqs.kt               # Arrow key escape sequences
│           └── TerminalSessionService.kt   # Foreground service + wake lock
├── daemon-rs/                  # zellandd daemon
│   └── src/
│       ├── main.rs             # axum server, CLI
│       ├── store/              # annotation storage (KDL)
│       ├── handlers/           # REST handlers
│       └── ws.rs               # WebSocket sync
├── libghostty/                 # libghostty-vt submodule
└── proto/zelland.proto         # Protobuf schema (prost 0.14.3)
```

---

## Building

### Android

```bash
# Prerequisites: Android SDK, NDK, Rust android targets
rustup target add aarch64-linux-android

cd src-tauri
npm install
npm run tauri android build
```

### Linux Desktop

```bash
cd src-tauri
npm install
npm run tauri dev
```

### Daemon

```bash
cd daemon-rs
cargo build --release
./target/release/zellandd --port 7700
```

---

## Testing

| Layer | Runner | Count |
|---|---|---|
| TypeScript utils | `npm run test` (Vitest) | ~60 tests |
| Rust src-tauri | `cargo test` in `src-tauri/` | ~12 tests |
| Rust daemon-rs | `cargo test` in `daemon-rs/` | ~58 tests |
| Kotlin unit | `./gradlew test` in `src-tauri/gen/android/` | ~25 tests |

See [`TESTING.md`](TESTING.md) for the full strategy and manual smoke test checklist.

---

## Docs

- [`WGPU_FIXES.md`](WGPU_FIXES.md) — wgpu + glyphon Android rendering architecture and known fixes
- [`CLAUDE_NOTES.md`](CLAUDE_NOTES.md) — code review notes and open issues
- [`TESTING.md`](TESTING.md) — test strategy and smoke test checklist

---

## License

```
Copyright 2026 zelland Contributors
Licensed under the Apache License, Version 2.0
```
