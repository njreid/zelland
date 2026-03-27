# Claude Code Review Notes
*Generated 2026-03-25 — covers src-tauri/src/{ghostty,terminal,renderer/*,ssh,lib}.rs + build.rs + MainActivity.kt*
*Updated 2026-03-27 — marked resolved items; added DrawerLayout sidebar architecture*

> **Status legend:** ✅ Resolved · ⚠️ Open · 🔲 Won't fix

---

## 1. Dead Code from Phase 1 / Hybrid Mode

### `render_viewport` + `render_buf` should be deleted
`TerminalSession::render_viewport` (terminal.rs) is the old ANSI-snapshot path from Phase 1.
`ssh.rs` only ever calls `ts.render_native()`, so `render_viewport` is never reachable.
`render_buf: Vec<u8>` (8 KiB pre-allocation per session) is therefore also dead.

**Fix:** Delete `render_viewport`, `render_buf`, and the field initialiser in `new()`.

### `SshChannelMsg::Viewport` is dead on the native path
The `Viewport { data, at_bottom, mouse_mode }` variant is still present and even has test coverage.
Once `render_viewport` is removed, nothing emits it.

**Fix:** Remove the variant (and update the test to cover `SshChannelMsg::Closed` or a real native event).

---

## 2. Stale / Misleading Comments ✅ Partially resolved

### `resize()` comment refers to Phase 1
```rust
// Pixel sizes are currently approximated or zeroed since we're in Phase 1 (JS rendering)
self.term.resize(cols, rows, 0, 0)
```
We are in Phase 3 (native wgpu rendering). Passing `(0, 0)` for pixel dimensions means Ghostty
cannot compute pixel-accurate mouse coordinates or font-metrics-based line heights.

**Fix:** Thread real pixel sizes through `SessionMsg::Resize` and the `SshManager::resize` command
so Ghostty gets `(cols * CELL_WIDTH, rows * CELL_HEIGHT)` instead of zeros.

### `scroll()` mentions Zellij ✅ Fixed
The comment now reads `// Zero-scrollback design: no local buffer to scroll.`

---

## 3. Panics Instead of Recoverable Errors ✅ Resolved

All hot-path panics have been replaced with graceful error handling:

| Location | Fix |
|---|---|
| `terminal.rs:TerminalSession::new` | Now returns `Result<Self, String>`; caller in `ssh.rs` logs error and returns from the task |
| `terminal.rs:render_native` | Returns `Result<(), String>`; already fixed |
| `renderer/mod.rs:render` | Logs error and returns early on surface texture failure |
| `renderer/mod.rs:Renderer::init` | Logs error and returns early on adapter/device failure |

---

## 4. Renderer: Text-Only, Styling Completely Absent ✅ Resolved

`build_row_runs` in `renderer/mod.rs` calls `get_cell_style()` per cell and extracts `fg_color`,
`bold`, `italic`, and `inverse` flags. Colors flow through `ghostty_color_to_rgb` (handles palette
indices and RGB values) into `CellRun.fg`. `draw_ghostty_state` passes `fg/bold/italic` to glyphon
`Attrs` via `set_rich_text`. A 16-color ANSI palette is mapped in `ansi_palette_color()`.

---

## 5. `row_cache` Never Shrinks on Resize

```rust
self.row_cache.resize(line_idx as usize + 1, String::new());
```
If the terminal shrinks (fewer rows), stale entries remain at the tail of `row_cache`.
On the next frame these will be sent to glyphon, rendering ghost lines below the active viewport.

**Fix:** In `draw_ghostty_state`, after the row loop, truncate `self.row_cache` to the actual row count.
Also clear the cache entirely on `passResizeToRust`.

---

## 6. `GhosttyMouseEncoder` Allocated Per Event

`encode_mouse_event` constructs and frees a `GhosttyMouseEncoder` + `GhosttyMouseEvent` on every
single touch. On a 60 Hz scroll this is 60 FFI alloc/free pairs per second.

**Fix:** Cache one `GhosttyMouseEncoder` in `TerminalSession` (similar to how `render_state` is cached),
reset it per call with `ghostty_mouse_encoder_setopt_from_terminal`.

---

## 7. Global `RENDERER` Singleton Architecture

`static RENDERER: Lazy<Mutex<Option<Renderer>>>` is a process-wide singleton.

Issues:
- **Silently drops frames**: `with_renderer` does nothing if the renderer isn't yet initialised.
  There is a TOCTOU window between `passSurfaceToRust` spawning the async init and the first
  `render_native()` call from the SSH loop.
- **Single-session limitation**: Only one wgpu surface is possible; a second tab cannot have its own renderer.
- **`Mutex` on render hot path**: `with_renderer` acquires a `std::sync::Mutex` inside a tokio task,
  which can block the executor thread if the renderer holds the lock across a slow GPU submit.

**Fix (short-term):** Change `Renderer::init` to be synchronous (wgpu `block_on`) or add a ready
flag so `render_native` can log a warning rather than silently skipping. Long-term, pass the renderer
handle through `SshManager` state rather than using a global.

---

## 8. `wgpu::Backends::VULKAN` Hard-Coded

```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN,
    ..Default::default()
});
```
This breaks on Android devices without Vulkan (Mali GPUs on Android 8/9, some emulators).
wgpu supports OpenGL ES fallback via `Backends::GL`.

**Fix:** Use `Backends::all()` (or `Backends::VULKAN | Backends::GL`) and let wgpu pick the best available.

---

## 9. Minor: `draw_terminal_grid` Debug String Leaked Into Production

`passSurfaceToRust` calls `renderer.draw_terminal_grid("Zelland Native Surface Initialized")` once
on surface creation. This is a placeholder "hello world" path — once `render_native` is wired up
it is never shown, but the method and its call still exist.

**Fix:** Remove `draw_terminal_grid` entirely; the first real `render_native` call is sufficient.

---

## 10. `unsafe impl Send/Sync` Correctness

`GhosttyTerminalWrapper` and `GhosttyRenderStateWrapper` are both declared `Send + Sync`.
The Ghostty C library documentation should be checked that these opaque pointers are either
thread-safe or that the usage guarantees single-threaded access. Currently `TerminalSession`
lives entirely within a single `tokio::spawn` task, so `Send` is needed but `Sync` is not.
Removing the `Sync` impl is the conservative choice until thread-safety is confirmed upstream.

---

## 11. SSH Sessions Die When the Screen Locks ✅ Fully resolved

Android aggressively restricts background CPU and kills processes when the screen turns off.
Without explicit OS contracts, the tokio runtime pauses, the SSH `select!` loop stalls, and
the server-side connection times out within seconds to minutes.

Three layers need to work together:

### 11a. Android Foreground Service ✅
A `Service` running in the foreground is the only reliable way to keep a process alive while
the screen is off. Without it, Android's OOM killer and the battery optimiser will terminate
the app.

What to add:
- A `TerminalSessionService` in Kotlin that extends `Service`, started with
  `startForeground(id, notification)` and the `FOREGROUND_SERVICE_TYPE_DATA_SYNC` (or
  `REMOTE_COMMUNICATION` on API 34+) type declared in `AndroidManifest.xml`.
- `MainActivity` binds to / starts the service when the first SSH tab opens and stops it
  when the last tab closes.
- `AndroidManifest.xml` needs `<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />`
  (and the `foregroundServiceType` attribute on the `<service>` element).
- The persistent notification is mandatory — it is what tells the user the app is holding a
  connection open.

### 11b. Partial Wake Lock ✅
Even inside a foreground service the CPU can sleep between I/O events on some vendors (Xiaomi,
Oppo, Samsung with aggressive power profiles). Acquiring a `PARTIAL_WAKE_LOCK` prevents this:

```kotlin
val pm = getSystemService(POWER_SERVICE) as PowerManager
val wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "zelland:ssh")
wakeLock.acquire()   // release in onDestroy / when last session closes
```

`PARTIAL_WAKE_LOCK` does not keep the screen on — only the CPU. It must be released when all
sessions are closed to avoid draining the battery.

### 11c. SSH-level Keepalives ✅
`ssh.rs` sets `keepalive_interval: Some(30s)`, `keepalive_max: 3` in `russh::client::Config`.

### 11d. Surface Destruction on Lock ✅
`SurfaceHolder.Callback` is implemented in `setupNativeSurface`. `surfaceCreated` calls
`passSurfaceToRust`; `surfaceDestroyed` calls `passSurfaceDestroyedToRust`. The Rust renderer
releases and recreates the wgpu surface accordingly.

### Summary — all layers implemented ✅

| Layer | File(s) | Status |
|---|---|---|
| Foreground service | `TerminalSessionService.kt`, `AndroidManifest.xml` | ✅ Done |
| Wake lock | `TerminalSessionService.kt` | ✅ Done |
| SSH keepalive | `src-tauri/src/ssh.rs` | ✅ Done |
| Surface lifecycle | `MainActivity.kt` (SurfaceHolder.Callback) | ✅ Done |
| Renderer teardown | `renderer/android.rs` (`passSurfaceDestroyedToRust`) | ✅ Done |

---

## Summary Table

| # | Status | Severity | File | Issue |
|---|---|---|---|---|
| 1 | ⚠️ Open | Medium | terminal.rs | Dead `render_viewport` + `render_buf` |
| 2 | ✅ Done | Low | terminal.rs, ssh.rs | Stale Phase 1 / Zellij comments |
| 3 | ✅ Done | High | terminal.rs, renderer/ | `.expect()` panics in hot paths |
| 4 | ✅ Done | High | renderer/mod.rs | Cell styles/colors completely ignored |
| 5 | ⚠️ Open | Medium | renderer/mod.rs | `row_cache` never shrinks on resize |
| 6 | ⚠️ Open | Low | terminal.rs | Per-event mouse encoder allocation |
| 7 | ⚠️ Open | Medium | renderer/ | Global singleton + silent frame drops |
| 8 | ⚠️ Open | Medium | renderer/mod.rs | Vulkan-only backend (breaks Android 8/9) |
| 9 | ⚠️ Open | Low | renderer/android.rs | Debug `draw_terminal_grid` still present |
| 10 | ⚠️ Open | Low | ghostty.rs | Unnecessary `Sync` impls on FFI wrappers |
| 11 | ✅ Done | High | Android lifecycle | Screen lock kills SSH sessions |
