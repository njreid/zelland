# wgpu / Android Rendering Architecture & Fixes

This document records the architecture decisions, bugs found, and fixes applied while
getting wgpu + glyphon + libghostty rendering and touch interaction working on Android.
Useful reference if the renderer is rebuilt or ported.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Android Activity (MainActivity.kt)                      │
│                                                          │
│  FrameLayout                                             │
│  ├── WebView  (added first → lower touch priority)       │
│  │   └── Svelte app (UI, sidebar, session list)          │
│  └── SurfaceView  (added second → higher touch priority) │
│      └── wgpu Vulkan surface (terminal rendering)        │
│                                                          │
│  GestureDetectorCompat (attached to SurfaceView)         │
│  KeybarPlugin (LinearLayout below viewport)              │
└─────────────────────────────────────────────────────────┘
         │ JNI                              │ JS bridge
         ▼                                  ▼
┌─────────────────┐              ┌──────────────────────┐
│  Rust (renderer)│              │  TerminalNative       │
│  passSurface    │              │  .setVisible(bool)    │
│  passResize     │              │                       │
│  passTouch      │              │  KeybarNative         │
└────────┬────────┘              │  .setVisible(bool)    │
         │                       └──────────────────────┘
         ▼
┌─────────────────────────────────────────────────────────┐
│  SshManager → TerminalSession → GhosttyTerminalWrapper   │
│                                                          │
│  VT bytes (SSH) → libghostty-vt → render state          │
│  Touch events → SGR mouse sequences → SSH channel       │
└─────────────────────────────────────────────────────────┘
```

---

## Fix 1: Atlas format must match the surface format

**Symptom:** Terminal surface is present and the wgpu clear colour is visible, but text is
completely invisible. No error is logged by `text_renderer.render()`.

**Root cause:** `glyphon::TextAtlas` constructed with a hardcoded texture format
(`Rgba8UnormSrgb`). Android Vulkan surfaces almost always report `Bgra8Unorm` as their
preferred format. When `text_renderer.render()` is called inside a render pass whose colour
attachment has a *different* format, the draw is silently discarded.

**Fix:** After `surface.get_capabilities(&adapter)`, take `caps.formats[0]` and rebuild
the atlas + TextRenderer if the format has changed:

```rust
let surface_format = caps.formats[0];
if surface_format != self.atlas_format {
    let mut atlas = TextAtlas::new(&device, &queue, &cache, surface_format);
    let text_renderer = TextRenderer::new(&mut atlas, &device,
        MultisampleState::default(), None);
    self.atlas = atlas;
    self.atlas_format = surface_format;
    self.text_renderer = text_renderer;
    self.row_cache.clear();
}
```

**Rule:** The format passed to `TextAtlas::new` must exactly match the format of every
render pass colour attachment that `text_renderer.render()` is called against.

---

## Fix 2: Call `render()` after applying the deferred resize in `set_surface()`

**Symptom:** After the surface is created, the screen stays black until the first SSH
session connects and triggers `render_native()`.

**Root cause:** `set_surface()` applies a stored `pending_size` via `self.resize(w, h)` but
did not call `self.render()` afterward, so the first frame was never submitted.

**Fix:**

```rust
if let Some((w, h)) = self.pending_size.take() {
    self.resize(w, h);
    self.render();   // draw the background so the surface is confirmed working
}
```

---

## Fix 3: SurfaceView covers WebView on startup

**Symptom:** The welcome screen / sessions list is completely inaccessible — the entire
WebView is hidden behind the SurfaceView immediately on launch.

**Root cause:** `currentPaneIndex` starts at 0 (terminal pane). The Svelte `$effect` that
calls `TerminalNative.setVisible(currentPaneIndex === 0)` fires immediately, making the
SurfaceView visible before the user has connected any session. Because the SurfaceView uses
`setZOrderMediaOverlay(true)`, it composites above the WebView and intercepts all touches.

**Fix (two parts):**

1. Start the SurfaceView as `GONE` in Kotlin:
   ```kotlin
   visibility = android.view.View.GONE
   ```

2. Guard the JS visibility call on an active session in Svelte:
   ```js
   (window as any).TerminalNative?.setVisible(currentPaneIndex === 0 && !!appState.activeSessionId);
   ```

---

## Fix 4: WebView consuming all touches, GestureDetector never firing

**Symptom:** Taps and two-finger scrolls on the terminal surface had no effect. No touch
logs appeared from the GestureDetector callbacks.

**Root cause:** Android touch dispatch goes to the *last-added* child of a ViewGroup first.
The SurfaceView was added to the FrameLayout before the WebView, meaning WebView was the
last added and therefore received all touches first — consuming them before the
SurfaceView's `OnTouchListener` could see them.

**Fix:** Swap the `addView` order so WebView is added first, SurfaceView second:

```kotlin
// WebView first → lower touch priority (last resort)
container.addView(webView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
// SurfaceView second → first to receive touches when VISIBLE
container.addView(surfaceView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
```

**Note:** `setZOrderMediaOverlay(true)` controls *compositor* z-order (visual layering),
NOT touch dispatch order. Touch dispatch is determined solely by the view hierarchy order.

---

## Fix 5: Scroll direction inverted

**Symptom:** Two-finger swipe up scrolled the terminal up (backwards).

**Root cause:** `GestureDetector.onScroll` `distanceY` is positive when the finger moves
**up** (i.e., content moves down / terminal scrolls forward). The initial mapping had this
backwards.

**Fix:**
```kotlin
// distanceY > 0 → finger moved up → content scrolls down
val action = if (distanceY > 0) "scroll_down" else "scroll_up"
```

---

## Touch Pipeline (end-to-end)

```
User touches SurfaceView
    → SurfaceView.OnTouchListener → GestureDetectorCompat.onTouchEvent()
    → GestureDetector.onSingleTapConfirmed / onScroll (2-finger only)
    → passTouchToRust(action, x, y)  [JNI call on UI thread]
    → renderer/android.rs: Java_..._passTouchToRust
    → spawn_on_runtime → SshManager.process_touch(action, x, y)
    → ssh.rs: send_to_session → SessionMsg::ProcessMouse
    → ssh.rs session loop: TerminalSession.process_mouse(x, y, action)
    → terminal.rs: encode_mouse_event() via libghostty C FFI
    → SGR sequence bytes → SSH channel.data() → remote zellij
```

**Important:** Mouse events are only forwarded if the terminal has mouse tracking enabled
(`\x1b[?1000h`). This sequence is sent by zellij shortly after attach. Events received
before this are silently dropped.

**SGR encoding:** Uses `ghostty_mouse_encoder_*` C FFI functions from libghostty-vt. The
encoder requires accurate pixel-to-cell mapping via `GhosttyMouseEncoderSize`. Cell
dimensions are read from the live renderer (`renderer::get_cell_size()`), not hardcoded
constants — the renderer updates them after font metrics are computed.

---

## Cell Size: Constants vs. Live Values

`CELL_WIDTH = 17.0` and `CELL_HEIGHT = 38.0` in `renderer/mod.rs` are compile-time
fallbacks only. After the renderer initialises and measures the actual font, it stores live
values in `renderer.cell_width` / `renderer.cell_height`.

`renderer::get_cell_size()` returns the live values if a renderer exists, or falls back to
the constants. Always use `get_cell_size()` for mouse coordinate mapping to avoid
off-by-one cell errors.

---

## libghostty-vt Integration

libghostty-vt is a C FFI library (from the Ghostty project) providing:

- **`GhosttyTerminalWrapper`** — VT sequence processor. Call `.write(bytes)` to feed SSH
  output; the terminal state (grid, cursor, attributes) is updated in-place.
- **`GhosttyRenderStateWrapper`** — snapshot of terminal state for rendering. Updated via
  `.update(&term)` before each frame.
- **Mouse encoder** — `ghostty_mouse_encoder_*` functions encode touch events into ANSI
  SGR sequences (`\x1b[<Cb;Cx;CyM/m` format) based on the current mouse mode and terminal
  geometry.

The render state is fed into `Renderer.draw_ghostty_state()` which iterates cells and
draws them via glyphon (text) and wgpu (backgrounds / cursor).

---

## Android SurfaceView / WebView Composition

- **`setZOrderMediaOverlay(true)`**: SurfaceView composites *above* the WebView layer.
  Required because Tauri's WebView uses hardware acceleration; the default SurfaceView
  punch-through mechanism does not work reliably in this setup.
- **WebView transparency**: `webView.setBackgroundColor(Color.TRANSPARENT)` makes the
  WebView background transparent so the wgpu terminal shows through the WebView when the
  WebView's content is transparent in that region.
- **Keyboard (IME)**: A hidden `EditText` (0×0, off-screen) receives focus on tap so the
  system keyboard attaches to it. Key events flow through Tauri's WebView input handling
  to the SSH channel.

---

## SurfaceView Lifecycle

```
surfaceCreated  → passSurfaceToRust (JNI) → Renderer::init() if needed → set_surface()
surfaceChanged  → passResizeToRust  (JNI) → renderer.resize(w, h) → renderer.render()
surfaceDestroyed→ passSurfaceDestroyedToRust (JNI) → renderer.drop_surface()
```

The surface is destroyed and recreated on activity resume, screen lock/unlock, and
sometimes on first launch as the window insets are applied. Always store `pending_size`
when resize arrives before the surface is ready.

---

## General Checklist for wgpu + glyphon on Android

1. **Surface format** — always take `caps.formats[0]`; never hardcode `Rgba8UnormSrgb`.
   Android Vulkan typically gives `Bgra8Unorm`.

2. **Atlas format** — must equal surface format. Rebuild atlas + TextRenderer whenever the
   surface format changes (every `set_surface()` call where the format differs).

3. **Viewport resolution** — update via `viewport.update(&queue, Resolution { width, height })`
   on every resize, *after* `surface.configure()`.

4. **Font loading** — Android SELinux may block fontdb's automatic font discovery. Manually
   try well-known paths: `/system/fonts/NotoSansMono-Regular.ttf`, `DroidSansMono.ttf`,
   `Roboto-Regular.ttf`, etc.

5. **max_texture_dimension_2d** — clamp resize dimensions to
   `device.limits().max_texture_dimension_2d`. Some Android GPUs report 2048, so a
   1080×2286 screen gets clamped to 1080×2048.

6. **View order** — in a FrameLayout, the last-added child receives touches first.
   Add WebView first, SurfaceView second, so the SurfaceView wins touches when visible.

7. **SurfaceView Z-order** — `setZOrderMediaOverlay(true)` for visual layering above WebView.
   Start as `GONE`; use a JS bridge to show/hide based on active session + pane index.

8. **Surface lifecycle** — handle `surfaceCreated`, `surfaceChanged`, `surfaceDestroyed`;
   store `pending_size` if the renderer is not yet ready when resize arrives.

9. **Mouse mode guard** — only forward touch events to the SSH channel after the terminal
   has enabled mouse tracking (`\x1b[?1000h`). Check `get_mouse_tracking()` before encoding.

10. **Cell dimensions** — use `renderer::get_cell_size()` for mouse coordinate mapping,
    not compile-time constants. The renderer updates them after font initialisation.
