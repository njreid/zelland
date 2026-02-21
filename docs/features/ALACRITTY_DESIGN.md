# Design: Rust-side Terminal Emulation with termwiz

This document describes the architecture for moving terminal state management
from the frontend (xterm.js) into the Rust backend, eliminating the
JavaScript VT parser and replacing xterm.js's renderer with a lightweight
Canvas/WebGL renderer driven directly by Rust.

## Goal

Use `termwiz` to own the terminal grid in Rust. The frontend becomes a pure
renderer: it receives structured cell-update events and draws them to a
`<canvas>` element. xterm.js is removed entirely.

## Prior Art: Rio Terminal

[Rio Terminal](https://raphamorim.io/rio/) is the closest reference
implementation. Rio uses `wgpu` + its own `sugarloaf` rendering library and
has a working WASM build (`rio-wasm`) that runs the full terminal emulator in
a browser via WebGPU. The data flow is identical to what we want:

```
PTY bytes → VT parser → grid state → renderer → frame
```

In our case the renderer is Canvas2D/WebGL inside the Tauri WebView rather
than `sugarloaf`, which avoids creating a native wgpu surface that would live
outside the WebView and break annotation overlays.

## Why not a native wgpu surface

Tauri renders into a WebView. A native wgpu surface would sit outside the
WebView as a separate OS-level layer — you'd need to punch a transparent hole
through the WebView to expose it. This breaks the annotation overlay system,
the pane ribbon, and the sidebar, all of which composite freely over the
terminal today. **WebGL inside the WebView via `<canvas>`** achieves the same
GPU-accelerated rendering without sacrificing layout compositing.

## Why `termwiz` over `alacritty_terminal`

| | `alacritty_terminal` | `termwiz` |
|---|---|---|
| Design intent | Internal to Alacritty | Designed for embedding |
| Semver stability | None (breaks each minor) | Stable public API |
| Maintained by | Alacritty team | WezTerm (wez) |
| Scrollback API | Manual | Built-in `ScrollbackBuffer` |
| SIXEL / images | No | Yes |

`termwiz` exposes `termwiz::escape::parser::Parser` (VT state machine),
`termwiz::surface::Surface` (the grid), and `termwiz::surface::Change`
(the diff type). These three are everything we need.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Rust (src-tauri)                                       │
│                                                         │
│  russh channel                                          │
│       │ raw PTY bytes                                   │
│       ▼                                                 │
│  termwiz::escape::parser::Parser                        │
│       │ parsed Actions → Changes                        │
│       ▼                                                 │
│  termwiz::surface::Surface  (grid + cursor)             │
│  + VecDeque<Vec<Change>>  (scrollback)                  │
│       │ dirty rows only                                 │
│       ▼                                                 │
│  Tauri event  "terminal-render"                         │
│       │  { tab_id, rows: Vec<RowUpdate>, cursor }       │
└───────┼─────────────────────────────────────────────────┘
        │
┌───────▼─────────────────────────────────────────────────┐
│  Frontend (Terminal.svelte)                             │
│                                                         │
│  <canvas> element                                       │
│       ▲                                                 │
│  CanvasRenderer  (~300 lines TypeScript)                │
│    - glyph cache (one OffscreenCanvas per unique glyph) │
│    - row-dirty tracking, cursor, selection              │
│    - WebGL path on desktop; Canvas2D fallback Android   │
│                                                         │
│  <div class="overlay"> (transparent, pointer-events)    │
│    - annotation highlights (existing system, unchanged) │
└─────────────────────────────────────────────────────────┘
```

### Serialized row format

Keeping the IPC payload small matters. Each cell encodes to ~8 bytes:

```rust
struct SerializedCell {
    ch: char,       // 1–4 bytes UTF-8
    fg: [u8; 3],    // RGB
    bg: [u8; 3],    // RGB
    attrs: u8,      // bold | italic | underline | blink | reverse (bitfield)
}

struct RenderEvent {
    tab_id: String,
    rows: HashMap<u16, Vec<SerializedCell>>,  // only dirty rows
    cursor: Option<(u16, u16)>,               // (col, row)
}
```

Typical dirty repaint for a 220-column terminal: 3–5 rows × ~1800 bytes =
~9 KB. Full repaint: ~88 KB. Both are well within Tauri's IPC budget.

### Canvas renderer (TypeScript sketch)

```typescript
class CanvasRenderer {
    private cellW: number;
    private cellH: number;

    render(rows: Map<number, SerializedCell[]>, cursor: [number, number] | null) {
        for (const [rowIdx, cells] of rows) {
            this.renderRow(rowIdx, cells);
        }
        if (cursor) this.renderCursor(cursor);
    }

    private renderRow(row: number, cells: SerializedCell[]) {
        const y = row * this.cellH;
        for (let col = 0; col < cells.length; col++) {
            const { ch, fg, bg } = cells[col];
            const x = col * this.cellW;
            this.ctx.fillStyle = rgb(bg);
            this.ctx.fillRect(x, y, this.cellW, this.cellH);
            if (ch !== ' ') {
                this.ctx.fillStyle = rgb(fg);
                this.ctx.fillText(ch, x, y + this.cellH * 0.8);
            }
        }
    }
}
```

A full implementation with bold/italic variants, underline, and ligature
support via an OffscreenCanvas glyph atlas is ~300 lines. This is the same
approach WezTerm uses for its WebGL renderer.

## Implementation Steps

### Phase 1 — Rust: termwiz parser and grid

1. Add `termwiz = { version = "0.22", default-features = false, features = ["use_std"] }`
   to `src-tauri/Cargo.toml`.
2. In `ssh.rs`, introduce a `TermSession` per tab:
   ```rust
   struct TermSession {
       parser: termwiz::escape::parser::Parser,
       surface: termwiz::surface::Surface,
       scrollback: VecDeque<Vec<termwiz::surface::Change>>,
   }
   ```
3. In the SSH read loop, feed bytes into `parser.parse(bytes, |action| ...)`,
   apply resulting `Change`s to `surface`, collect dirty row indices, and
   emit a `terminal-render` Tauri event containing only the changed rows.
4. Add a Tauri command `terminal_scroll(tab_id, delta: i32)` that replays
   scrollback `Change`s to project a different viewport window, then
   re-emits the full visible surface.

### Phase 2 — Frontend: CanvasRenderer

5. Replace `<div bind:this={terminalElement}>` with `<canvas>` in
   `Terminal.svelte`.
6. Implement `CanvasRenderer` in `src/lib/terminal/renderer.ts`. Measure cell
   dimensions from a hidden `measureText` call on init.
7. Listen to `terminal-render` events, pass rows/cursor to `renderer.render()`.
8. Handle keyboard input via `canvas.addEventListener('keydown', ...)`,
   encoding keys to byte sequences and sending to `appState.writeInput()`.
9. Handle resize via `ResizeObserver` — compute `(cols, rows)` from
   `canvas.width / cellW` and call `appState.resize()`.
10. Remove `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl`,
    `@xterm/addon-canvas` from `package.json`.

### Phase 3 — Scrollback and selection

11. Scrollbar or scroll gesture in Svelte maps to `terminal_scroll`. Rust
    projects the correct window and re-emits the viewport.
12. Text selection: track mouse start/end cell coordinates, highlight on
    canvas, copy via `navigator.clipboard.writeText()`.

## Intermediate state (today)

While the migration is in progress, xterm.js runs with:

- `scrollback: 0` ✓ (already set — eliminates main Android memory pressure)
- WebGL renderer with Canvas2D fallback ✓ (added — ~2× faster rendering)

These address ~80% of the Android performance concern with no architecture
changes.

## Future Enhancements

- **Annotation integration:** The termwiz grid makes it trivial to locate
  text spans server-side, removing the fragile `prefix/suffix` heuristic
  used today.
- **Search:** Regex search across full scrollback via the `regex` crate in
  Rust; results arrive as `(row, col_start, col_end)` spans.
- **Semantic selection:** Unicode word-break detection in Rust on double-click,
  far more accurate than xterm.js's heuristics.
- **SIXEL / image support:** termwiz supports SIXEL natively; the canvas
  renderer composites image cells as `drawImage` calls.
