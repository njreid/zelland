# Testing Strategy & Guidelines

*Updated 2026-03-27 — reflects current stack: Tauri v2 + Svelte 5 + Rust SSH + wgpu/glyphon native terminal + Android.*

## What exists today

| Layer | Files | Count | Runner |
|---|---|---|---|
| TypeScript utils | `src/lib/utils/*.test.ts` | 5 files, ~60 tests | Vitest |
| Rust (daemon-rs) | `daemon-rs/src/**/*.rs` | ~58 tests | `cargo test` |
| Rust (src-tauri) | `src-tauri/src/{ssh,terminal,network,keystore,lib}.rs` | ~12 tests | `cargo test` |
| Kotlin (unit) | `KeybarSeqsTest.kt`, `KeySeqsTest.kt` | ~25 tests | JUnit 4 |

---

## 1. TypeScript Utilities (Vitest)

Run with: `npm run test` or `npx vitest run`

### What's tested
- `key-mapper.test.ts` — keycode → terminal sequence mapping
- `kb-input.test.ts` — keyboard input pipeline (modifiers, special keys)
- `md-ordering.test.ts` — markdown file sort order
- `markdown-path.test.ts` — path normalization utilities
- `time-ago.test.ts` — relative timestamp formatting (8 boundary cases)

### What to add next
- `app.svelte.ts` store logic — `buildSshConfig()` with each `AuthMethod` variant
- Sidebar data shape — verify JSON sent to `SidebarNative.updateData()` matches the
  expected `{sessions, hosts, activeSessionId}` structure
- `KeybarPlugin` sequence generation for special combos (Ctrl+Enter, Meta+Arrow)

---

## 2. Rust — `src-tauri` (cargo test)

Run with: `cargo test` in `src-tauri/`.

### What's tested
- `ssh.rs` — `AuthMethod` serde round-trips, `SshChannelMsg::Closed` serialization
- `terminal.rs` — `TerminalSession` construction, basic write/resize
- `network.rs` — WireGuard peer config parsing
- `keystore.rs` — key generation and retrieval stubs
- `lib.rs` — Tauri command registration smoke test

### What to add next (priority order)

**High value, low effort:**
- `terminal.rs` — `encode_mouse_event` SGR output: assert `\x1b[<0;C;RM` format for a
  click at a known (col, row) with mouse tracking enabled
- `terminal.rs` — `process_mouse` with `"scroll_up"` / `"scroll_down"` actions produces
  the correct SGR wheel sequences
- `renderer/mod.rs` — `extract_text` range clipping: `(0,0)→(0,5)` on a known row returns
  the first 5 characters; reversed coords return empty string

**Medium effort:**
- `renderer/mod.rs` — `update_selection_vertices` pixel→NDC math: given a 1080×2340
  surface and a 1-cell selection, assert the 6 vertex positions are within [-1, 1]
- `ssh.rs` — `SshManager::process_touch` when `focused_session` is None returns `Err`

---

## 3. Rust — `daemon-rs` (cargo test)

Run with: `cargo test` in `daemon-rs/`.

~58 tests covering config parsing, project listing, asset TTL, WebSocket sync,
file watcher, REST handlers, and YJS annotation sync. These are well-covered.

### What to add next
- `store/mod.rs` — annotation round-trip: write annotation → reload from disk → assert
  parsed fields match
- `handlers/sessions.rs` — recent sessions list is sorted by `connectedAt` descending

---

## 4. Kotlin — Android Unit Tests (JUnit 4)

Run with: `./gradlew test` in `src-tauri/gen/android/`.

### What's tested
- `KeybarSeqsTest.kt` — modified arrow key escape sequences (base + Ctrl/Alt/Meta combos)
- `KeySeqsTest.kt` — IME char → terminal sequence: plain chars, Ctrl+letter (0x01–0x1A),
  Ctrl+punctuation, Alt prefix, newline→CR

### What to add next

**Good candidates (pure logic, no Android framework needed):**
- `PixelToCellTest` — extract `pixelToCell` math (`x / cw`, `y / ch`) to a utility and
  test boundary conditions: `x=0`, `x=cw-1`, `x=cw`, off-screen coordinates
- `SidebarJsonTest` — extract JSON parsing from `updateNativeSidebarData` to a pure
  `SidebarData.parse(json)` function; test malformed JSON, missing fields, empty arrays
- `KeybarPluginSeqsTest` — test `KeybarPlugin.modCtrl / modAlt / modMeta` state reset
  after a sequence is emitted

**Requires Robolectric (heavier):**
- `DrawerLayoutOpenTest` — assert drawer opens on left-fling velocity threshold
- `SelectionActionModeTest` — long-press → ActionMode created with Copy/Paste items

---

## 5. Android Instrumented Tests (androidTest)

Not yet set up. Add `src-tauri/gen/android/app/src/androidTest/` when hardware/emulator
testing is needed.

### Recommended smoke tests
1. Surface lifecycle: `surfaceCreated` → first frame rendered (no black screen)
2. Orientation change: terminal resizes without crash
3. Drawer open/close: swipe-from-left opens sidebar, back press closes it
4. Keyboard: tap on terminal → IME appears; type character → SSH channel receives byte

---

## 6. CI

```yaml
# Rust tests
- run: cargo test --workspace

# TypeScript tests
- run: npm run test
- run: npm run check

# Kotlin unit tests
- run: ./gradlew test
  working-directory: src-tauri/gen/android
```

## 7. Manual Smoke Test (critical path)

1. Launch app on Android device
2. App shows welcome screen (not black SurfaceView)
3. Swipe left from edge → native sidebar opens
4. Tap "+ Session" → bottom-sheet form appears
5. Enter SSH credentials → "Create & Connect"
6. Terminal renders with cursor visible
7. Type `ls` → output appears; Ctrl+C → interrupt works
8. Two-finger scroll → terminal scrolls
9. Long-press → selection highlight + Copy/Paste action bar
10. Pinch → font size changes
11. Lock screen → unlock → terminal reconnects (or session resumes via foreground service)
12. Swipe right → markdown pane loads (if project active)
