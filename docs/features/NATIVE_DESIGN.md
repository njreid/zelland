# Native Terminal Design: Analysis of Gemini Research Notes

This document evaluates the architectural recommendations from the Gemini conversation
("Terminal Emulators: Architecture and Android Port") against zelland's actual stack
and proposes a concrete path forward.

---

## 1. Where the Notes Go Wrong

The Gemini thread produces solid general reasoning, but operates under a fundamental
false premise: **it assumes zelland is a native Android app (Kotlin + NDK + JNI)**.
zelland is a **Tauri v2** application. That changes nearly every conclusion.

### 1.1 Ghostty-VT via Zig Cross-Compilation — Unnecessary

The notes spend the most effort here.  Ghostty-VT is a headless terminal emulation
engine — exactly what zelland needs.  But zelland already **has** one:

```toml
# src-tauri/Cargo.toml  (already merged)
alacritty_terminal = "0.25.1"
```

`alacritty_terminal` is functionally equivalent to Ghostty-VT for this use case:
- Pure Rust, no Zig toolchain, no NDK cross-compile ceremony
- Compiles automatically as part of `cargo tauri android build`
- Already wired into `ssh.rs` — `TerminalSession` wraps `Term<TermEventProxy>`
  and `Processor` right now
- Alacritty's VT parser is arguably more battle-tested than Ghostty's (older
  codebase, wider deployment)

The setup.sh / Zig build.zig path would add months of maintenance surface for zero
user-visible benefit.

### 1.2 JNI Bridge — Already Provided by Tauri

The notes describe writing C++/Zig JNI glue so that Kotlin can call the native
engine.  In a Tauri app, the framework generates that bridge automatically.  Rust
code in `src-tauri/` is compiled into a `.so` that Tauri loads on Android; Kotlin
calls it via generated Tauri commands.  The `jni = "0.21"` crate is in Cargo.toml
only for the biometric/keystore bridge — all terminal work stays inside Rust.

### 1.3 Vulkan / SurfaceView Rendering — Wrong Layer

Rendering is handled by xterm.js inside the WebView, using the WebGL addon (with
Canvas fallback).  This is already done:

```svelte
// Terminal.svelte
const webgl = new WebglAddon();
term.loadAddon(webgl);
```

xterm.js WebGL is well within the Pixel 9's capabilities.  Dropping down to Vulkan
would mean rewriting the entire rendering layer in C++ with no benefit over the
existing WebGL path, and would lose xterm.js's font shaping, Unicode normalization,
and ligature support.

### 1.4 Telnet over WireGuard — Incorrect

The notes propose replacing SSH with raw TCP (Telnet) over WireGuard on the grounds
that WireGuard handles encryption at L3, so the application layer can be plain text.
This is wrong for zelland because:

- **No authentication**: Anyone on the WireGuard mesh could connect to the Telnet
  listener.  WireGuard authenticates *peers*, not *applications*.  A stray process on
  a shared dev machine could hijack the session.
- **No multiplexing**: SSH channels let us run a command, resize a PTY, and check
  host fingerprints on a single connection.  Telnet is one unbounded byte stream.
- **Already implemented**: `russh` handles SSH natively in Rust, cross-compiles to
  Android, and is already shipping.  Replacing it with a raw TCP socket adds work,
  not saves it.

SSH over WireGuard is the right answer and is the current implementation.

### 1.5 `com.wireguard.android:tunnel` — Redundant

The notes suggest bundling the WireGuard Android library (Go-based userspace
implementation compiled to Kotlin/JNI).  zelland already ships `gotatun`:

```toml
gotatun = { version = "0.2.0", features = ["device", "tun"] }
```

This is a Rust userspace WireGuard implementation that compiles within the Tauri
build.  Adding a second WireGuard stack (Go-backed, Kotlin API) would duplicate
functionality and add ~10MB of Go binary to the APK.

---

## 2. What the Notes Get Right

### 2.1 Roaming Tolerance via WireGuard (Already Done)

Correct.  WireGuard's kernel-level (or userspace) roaming means the SSH session
survives IP changes transparently.  zelland already has this.

### 2.2 Protobuf Push Notifications over TCP — Genuinely Good

This is the most valuable new idea in the thread.  Because zelland has:
- A direct WireGuard IP route to the phone
- Proto infrastructure (`proto/zelland.proto`, `prost = "0.14.3"`)
- A running daemon (`zellandd`) on the dev host

...a lightweight TCP push channel from daemon → app for notifications is an elegant
fit.  No FCM, no Google servers, no polling.

The suggested schema (session_name, tab_index, pane_id) maps naturally to Zellij's
environment variables (`$ZELLIJ_SESSION_NAME`, `$ZELLIJ_TAB_INDEX`,
`$ZELLIJ_PANE_ID`).

### 2.3 Multi-Session Parallel Connections — Backend Already Capable

`SshManager` stores sessions by `tab_id` in a `HashMap`:

```rust
pub active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<SessionMsg>>>>
```

Multiple connections can coexist today.  The gap is in the frontend UX and in
hooking per-session state (cursor position, viewport) to the right notification
deep-link.

### 2.4 Session-Aware Navigation on Notification Tap — Worth Building

Using notification `Intent` extras to navigate to the right terminal tab is the
correct Android pattern.  zelland's `app.svelte.ts` store already tracks `tabId`,
so the plumbing exists; the notification receiver just needs to dispatch a tab-switch
action.

### 2.5 UX: Mixed Content Ribbon with Exposé Overview

The PageView + pinch-to-Exposé concept maps directly onto zelland's existing "snap
scroll" ribbon (`src/routes/+page.svelte`).  A three-finger-pinch triggering a grid
of live tab thumbnails is achievable via the alacritty grid state zelland already
maintains in Rust — each background session is rendering into its `TerminalSession`
even when off-screen.

---

## 3. Recommended Solution for zelland

Keep the Tauri/Rust/xterm.js stack.  Extend three areas:

### 3.1 Protobuf Notification Push Channel

**On the daemon (`zellandd`)**:
- Add a `NotificationServer` that listens on a configurable TCP port (default: 7778)
  within the WireGuard interface.
- Expose a CLI helper (e.g., `znote`) that reads Zellij env vars and encodes a
  `ZellijNotification` proto message, then writes it to the port.

```proto
// proto/zelland.proto — extend existing file
message ZellijNotification {
  string title = 1;
  string body = 2;
  string session_name = 3;
  uint32 tab_index = 4;
  uint32 pane_id = 5;
  string command_preview = 6;
}
```

**On the Tauri client (Rust)**:
- Add a `NotificationListener` task that binds a TCP listener on the WireGuard
  interface address, receives proto-framed `ZellijNotification` messages, and calls
  `tauri_plugin_notification` to post a system notification.
- Store `session_name` + `tab_index` in the notification's action payload.

**On tap** (Kotlin `MainActivity` / Tauri intent handling):
- `intent::handle_notification_tap` dispatches a Tauri event with session/tab
  context, and `app.svelte.ts` switches to the matching tab, sending
  `zellij action go-to-tab` over the SSH channel.

### 3.2 Multi-Session Frontend Enhancement

The backend already supports N sessions.  Frontend work needed:

- Replace the single-tab terminal view with a scrollable tab strip (Svelte 5 rune
  for `activeSessions: SshTabState[]`).
- On notification tap: if the target `session_name` differs from the current SSH
  config, offer a one-tap "switch session" that connects (or re-attaches) to the
  named Zellij session.
- Background sessions continue parsing bytes via `TerminalSession` — use the grid
  state to render small live thumbnails in the Exposé overview.

### 3.3 Edge-Zone Gesture Routing

Already partially done (`isScrolling` flag in `Terminal.svelte`).  Extend:

- Reserve the left/right 12% of the screen width for swipe-to-switch-tab.
- Inner 76%: pass all touch events to xterm.js (scroll, mouse reporting, selection).
- Bottom edge: swipe up triggers the thumbnail Exposé grid (fullscreen → grid layout
  CSS transition).

---

## 4. Decision Matrix

| Concern | Gemini Recommendation | zelland Reality | Action |
|---|---|---|---|
| Terminal engine | Ghostty-VT (Zig cross-compile) | `alacritty_terminal` already present | **No change needed** |
| JNI bridge | Custom C++/Zig JNI | Tauri provides this automatically | **No change needed** |
| Rendering | Vulkan SurfaceView | xterm.js WebGL addon | **No change needed** |
| SSH transport | Telnet over WireGuard | `russh` over WireGuard | **Keep SSH** |
| WireGuard bundling | `com.wireguard.android:tunnel` (Go) | `gotatun` (Rust) | **No change needed** |
| Push notifications | TCP + Protobuf | Not yet implemented | **Build this** |
| Multi-session | Parallel Ghostty instances (new) | `SshManager` HashMap already supports it | **Frontend UX only** |
| Navigation deep-link | Intent extras | Tauri event system | **Wire up** |
| UX pattern | PageView + Exposé grid | Snap-scroll ribbon already exists | **Extend existing** |

---

## 5. Out of Scope

- **Mosh**: The PLAN.md mentions Mosh for roaming, but WireGuard + SSH already
  provides roaming tolerance.  Mosh adds complexity for marginal gain given the
  tunnel.
- **Pure-Zig SSH**: No production-ready implementation exists as of early 2026.
  `russh` is stable and cross-compiles cleanly.
- **16KB page alignment ceremony**: Cargo + Android NDK r27+ handles this
  automatically for Rust targets.  No special flags needed.
