# Design: Alacritty Terminal Backend Integration

This document describes the architecture for offloading terminal emulation logic from the frontend (JavaScript/xterm.js) to the backend (Rust/alacritty_terminal).

## Goal

Use the `alacritty_terminal` crate to maintain the terminal state (grid, scrollback, modes) in Rust, while using `xterm.js` as a high-performance rendering layer in the frontend.

## Motivation & Performance Analysis

### Linux Desktop

- **CPU Efficiency:** `alacritty_terminal` is one of the fastest terminal emulators available. Offloading escape sequence parsing to Rust frees up the main thread in the browser/Tauri webview.
- **Unified State:** Having the terminal grid available in Rust allows for advanced features like system-wide search, semantic selection, and integration with the Zelland daemon without round-trips to the frontend.

### Android

- **Memory Management (Critical):** `xterm.js` memory consumption grows significantly with scrollback history. On Android, memory is a constrained resource. By moving scrollback to Rust, we can set `xterm.js` scrollback to 0 and manage the history in Rust's heap (or even mmap it if needed), which is far more efficient than the V8/JSC heap.
- **Battery Life:** Parsing complex terminal escapes in Rust is more energy-efficient than doing so in JavaScript.
- **Responsiveness:** Heavy terminal output (e.g., `cat`ing a large log file) can freeze the JS thread. Rust can process this output at native speed and only send throttled "render" updates to the UI.

## Architecture

### 1. Backend (Rust)

The `SshManager` will be updated to own a `TerminalSession` for each tab.

```rust
struct TerminalSession {
    // Terminal state machine
    term: alacritty_terminal::Term<EventProxy>,
    // PTY/SSH handle
    channel: russh::client::Channel<Msg>,
    // Scrollback/History manager
    history: HistoryBuffer,
}
```

- **Input Path:** Raw bytes from `russh` -> `term.process_input(bytes)`.
- **State:** `alacritty_terminal` maintains the `Grid`, `Cursor`, and `Colors`.
- **Syncing:** When the grid is "dirty", the backend emits a specialized event to the frontend.

### 2. Frontend (TypeScript)

`Terminal.svelte` will continue to use `xterm.js`, but with its own emulation logic largely bypassed.

- **Scrollback:** Set `scrollback: 0`.
- **Rendering:** Instead of receiving a raw stream of SSH data, it receives "View Updates" from Rust.
- **User Interaction:** Key presses are still captured by `xterm.js` and sent to Rust via `ssh_write`.

### 3. Sync Protocol

To keep the frontend in sync without re-implementing a full renderer, we can use one of two strategies:

1. **Virtual Viewport:** Rust sends the raw bytes that *only* represent the current visible window. When the user scrolls, Rust sends a "clear and redraw" sequence for the new window.
2. **Cell Diffing:** Rust sends a binary representation of the changed cells (coordinates, character, attributes). A custom `xterm.js` addon renders these directly to the buffer.

**Preferred Strategy:** Strategy 1 is easier to implement initially as it leverages `xterm.js`'s existing robust escape sequence rendering.

## Implementation Steps

1. **Add Dependencies:** Add `alacritty_terminal` to `src-tauri/Cargo.toml`.
2. **Refactor `ssh.rs`:**
    - Create a `Term` instance per connection.
    - Implement `alacritty_terminal::event::EventListener` to catch terminal events (titles, bells, etc.).
    - Update the `tokio::select!` loop to feed `channel.wait()` data into the `Term`.
3. **Expose Grid API:** Add a Tauri command to "read" a range of the terminal grid.
4. **Update `Terminal.svelte`:**
    - Disable `xterm.js` scrollback.
    - Handle a new `terminal-sync` event that contains the visible area.
5. **Scrollback Bridge:** Implement a scrollbar in Svelte that tells the Rust backend which part of the history to "project" into the `xterm.js` viewport.

## Future Enhancements

- **Annotation Integration:** Use the Rust-side grid to automatically find and highlight text matching active annotations, even if they move due to terminal output.
- **Search:** Implement ultra-fast Regex search across the entire scrollback buffer in Rust.
- **Multiplexing:** Easier implementation of "split panes" or "overlay" features by managing multiple `Term` instances in one SSH session.
