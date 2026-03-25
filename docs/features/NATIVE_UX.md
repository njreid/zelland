# Native Android UX — Native Top Bar (Keyboard Plugin)

## Overview

The current `VirtualKeyboard.svelte` is a WebView-rendered component fixed at the bottom of the screen using the Visual Viewport API. Moving it to a **native Android view** positioned at the **top of the app window** would remove all system-keyboard interaction complexity, give true native touch responsiveness, and let the WebView occupy the full viewport without layout hacks.

This document explores the architecture, implementation path, open questions, and trade-offs.

---

## Motivation

### Problems with the current approach

| Problem | Root cause |
|---|---|
| Bar hidden by system keyboard | WebView shrinks under `adjustResize`; fixed-position elements misbehave |
| Visual Viewport API workaround | `window.visualViewport` listener updates `bottom` CSS to float above IME — fragile across Android versions |
| `100dvh` requirement | Needed so the WebView itself knows to shrink |
| `padding-bottom` on content | Must equal the keyboard bar height to avoid rendering behind it |
| JS-layer touch latency | Button presses go: touch → browser gesture recognizer → Svelte event handler → `appState.writeInput` |
| No haptic feedback on key press | Would need an extra `invoke("haptics_impact")` call per key |

### Benefits of a native top bar

- **Always visible.** The system keyboard rises from the bottom; a top bar is completely unaffected.
- **No Visual Viewport API.** The WebView fills the space below the bar; no dynamic CSS.
- **Native touch latency.** View's `OnTouchListener` or `View.OnClickListener` fires before the WebView even sees the event.
- **Hardware-accelerated ripples.** `RippleDrawable` / Material button for free.
- **Native haptics.** `VibrationEffect.createOneShot` or `HapticFeedbackConstants` — no bridge round-trip.
- **Survives WebView reload.** If the Svelte app crashes or hot-reloads, the bar persists.

---

## Current Architecture

```text
┌─────────────────────────────┐
│  Activity (TauriActivity)   │
│  ┌───────────────────────┐  │
│  │     WebView           │  │
│  │  ┌─────────────────┐  │  │
│  │  │  Svelte app     │  │  │
│  │  │  (xterm, MD…)   │  │  │
│  │  │                 │  │  │
│  │  │  VirtualKeyboard│  │  │  ← position: fixed; bottom: Npx
│  │  │  (CSS overlay)  │  │  │    driven by visualViewport API
│  │  └─────────────────┘  │  │
│  └───────────────────────┘  │
│  [ system keyboard ]        │  ← pushes WebView up via adjustResize
└─────────────────────────────┘
```

---

## Proposed Architecture

```text
┌─────────────────────────────┐
│  Activity (TauriActivity)   │
│  ┌───────────────────────┐  │  ← Native LinearLayout, added by plugin
│  │  [←][↑][↓][→]  right  │  │    arrow subbar: GONE by default,
│  │  aligned               │  │    VISIBLE when arrows toggled on
│  ├───────────────────────┤  │
│  │  [C][A][M] [1][2][3]  │  │    main bar: always visible
│  │  [ESC][⇥][⤢][↵]       │  │    ⤢ = arrows toggle button
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │     WebView           │  │  ← top padding = collapsed bar height;
│  │  (terminal, markdown) │  │    expands when arrow subbar is shown
│  │                       │  │
│  └───────────────────────┘  │
│  [ system keyboard ]        │  ← only affects WebView; bar is unaffected
└─────────────────────────────┘
```

---

## Tauri Plugin Architecture

Tauri v2 supports Android plugins as Kotlin classes that extend `app.tauri.plugin.Plugin`. The plugin lifecycle provides access to both the `Activity` and the `WebView` instance.

### Plugin structure

```text
src-tauri/
  src/
    keybar.rs          ← Rust plugin definition, command handlers
  gen/android/
    app/src/main/
      java/com/njr/zelland/
        KeybarPlugin.kt      ← Kotlin view management + event triggering
      res/layout/
        native_keybar.xml    ← XML layout for the bar
      res/drawable/
        kb_button_bg.xml     ← ripple drawable for buttons
```

### Rust side (`keybar.rs`)

```rust
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime, AppHandle, Emitter,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new("keybar")
        .setup(|_app, _api| Ok(()))
        .build()
}
```

The Rust side is minimal — key events flow Kotlin → JS via plugin `trigger()`, not through Rust.

### Kotlin side (`KeybarPlugin.kt`)

```kotlin
@TauriPlugin
class KeybarPlugin(private val activity: Activity) : Plugin(activity) {

    private var keybarView: View? = null
    private var arrowSubbar: View? = null
    private var webViewRef: WebView? = null
    private var modCtrl = false
    private var modAlt  = false
    private var modMeta = false
    private var arrowsOpen = false

    override fun load(webView: WebView) {
        super.load(webView)
        webViewRef = webView
        activity.runOnUiThread { setupKeybar(webView) }
    }

    private fun setupKeybar(webView: WebView) {
        val inflater = LayoutInflater.from(activity)
        val contentFrame = activity.window.decorView
            .findViewById<FrameLayout>(android.R.id.content)

        // Inflate the two-row container (main bar + arrow subbar)
        val bar = inflater.inflate(R.layout.native_keybar, contentFrame, false)
        keybarView = bar
        arrowSubbar = bar.findViewById(R.id.kb_arrow_row)

        // Arrow subbar starts hidden
        arrowSubbar?.visibility = View.GONE

        // Add bar at the top of the content frame
        contentFrame.addView(bar, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.WRAP_CONTENT,
            Gravity.TOP
        ))

        // After layout, push the WebView down so it doesn't sit under the bar.
        // Re-runs whenever visibility changes (arrow subbar toggle).
        bar.viewTreeObserver.addOnGlobalLayoutListener {
            val barHeight = bar.height
            webView.setPadding(0, barHeight, 0, 0)
        }

        setupButtons(bar)
    }

    private fun setupButtons(bar: View) {
        bar.findViewById<Button>(R.id.kb_ctrl).setOnClickListener {
            modCtrl = !modCtrl; updateModifierUI(bar)
        }
        bar.findViewById<Button>(R.id.kb_alt).setOnClickListener {
            modAlt = !modAlt; updateModifierUI(bar)
        }
        bar.findViewById<Button>(R.id.kb_meta).setOnClickListener {
            modMeta = !modMeta; updateModifierUI(bar)
        }

        // Arrows toggle — shows/hides the subbar row below the main bar
        bar.findViewById<ImageButton>(R.id.kb_arrows_toggle).setOnClickListener {
            arrowsOpen = !arrowsOpen
            arrowSubbar?.visibility = if (arrowsOpen) View.VISIBLE else View.GONE
            // WebView padding updates automatically via the GlobalLayoutListener
            updateArrowsToggleUI(bar)
        }

        bar.findViewById<Button>(R.id.kb_esc).setOnClickListener   { sendSeq("\u001b") }
        bar.findViewById<Button>(R.id.kb_enter).setOnClickListener { sendSeq("\r") }
        bar.findViewById<Button>(R.id.kb_tab).setOnClickListener   { sendSeq("\t") }

        // Arrow buttons in the subbar
        bar.findViewById<ImageButton>(R.id.kb_left).setOnClickListener  { sendArrow("D") }
        bar.findViewById<ImageButton>(R.id.kb_up).setOnClickListener    { sendArrow("A") }
        bar.findViewById<ImageButton>(R.id.kb_down).setOnClickListener  { sendArrow("B") }
        bar.findViewById<ImageButton>(R.id.kb_right).setOnClickListener { sendArrow("C") }
    }

    private fun sendSeq(seq: String) {
        val data = JSObject()
        data.put("seq", seq)
        trigger("kb-input", data)
        resetModifiers()
        activity.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    private fun sendArrow(letter: String) {
        var mod = 0
        if (modAlt || modMeta) mod += 2
        if (modCtrl) mod += 4
        val seq = if (mod == 0) "\u001b[$letter"
                  else "\u001b[1;${mod + 1}$letter"
        sendSeq(seq)
    }

    private fun resetModifiers() {
        modCtrl = false; modAlt = false; modMeta = false
        activity.runOnUiThread { keybarView?.let { updateModifierUI(it) } }
    }
}
```

### JavaScript side

```typescript
// In app.svelte.ts or Terminal.svelte onMount:
await listen<{ seq: string }>("kb-input", ({ payload }) => {
    if (appState.activeSessionId) {
        const bytes = new TextEncoder().encode(payload.seq);
        appState.writeInput(appState.activeSessionId, Array.from(bytes));
    }
});
```

The `VirtualKeyboard.svelte` component and its `position: fixed` wrapper in `+page.svelte` are removed entirely on Android (guarded by `isLinux` / platform check).

---

## Layout XML sketch

```xml
<!-- res/layout/native_keybar.xml -->
<!--
  Two-row vertical LinearLayout:
    Row 1 (arrow_row): right-aligned arrow keys, visibility=GONE by default
    Row 2 (main_bar):  always-visible modifier + special keys
  The GlobalLayoutListener in KeybarPlugin re-applies WebView top padding
  whenever the combined height changes (i.e. when arrow_row toggles).
-->
<LinearLayout
    android:layout_width="match_parent"
    android:layout_height="wrap_content"
    android:orientation="vertical"
    android:background="@color/kb_background">

    <!-- Arrow subbar: hidden until arrows toggle is pressed -->
    <LinearLayout
        android:id="@+id/kb_arrow_row"
        android:layout_width="match_parent"
        android:layout_height="40dp"
        android:orientation="horizontal"
        android:gravity="end|center_vertical"
        android:paddingHorizontal="4dp"
        android:visibility="gone">

        <ImageButton android:id="@+id/kb_left"  style="@style/KbKey" ... />
        <ImageButton android:id="@+id/kb_up"    style="@style/KbKey" ... />
        <ImageButton android:id="@+id/kb_down"  style="@style/KbKey" ... />
        <ImageButton android:id="@+id/kb_right" style="@style/KbKey" ... />
    </LinearLayout>

    <!-- Main bar: always visible -->
    <HorizontalScrollView
        android:layout_width="match_parent"
        android:layout_height="40dp"
        android:scrollbars="none">

        <LinearLayout
            android:layout_width="wrap_content"
            android:layout_height="match_parent"
            android:orientation="horizontal"
            android:padding="4dp"
            android:gravity="center_vertical">

            <!-- Left: menu + modifiers -->
            <ImageButton android:id="@+id/kb_menu" style="@style/KbKey" ... />
            <Button android:id="@+id/kb_ctrl" android:text="C" style="@style/KbMod" />
            <Button android:id="@+id/kb_alt"  android:text="A" style="@style/KbMod" />
            <Button android:id="@+id/kb_meta" android:text="M" style="@style/KbMod" />

            <!-- Center: tab buttons (hidden via visibility on narrow screens) -->
            <Button android:id="@+id/kb_tab1" android:text="1" style="@style/KbTab" />
            <Button android:id="@+id/kb_tab2" android:text="2" style="@style/KbTab" />
            <Button android:id="@+id/kb_tab3" android:text="3" style="@style/KbTab" />

            <!-- Right: ESC, Tab, arrows toggle, Enter -->
            <Button      android:id="@+id/kb_esc"           android:text="ESC" style="@style/KbKey" />
            <ImageButton android:id="@+id/kb_tab"                              style="@style/KbKey" ... /> <!-- ⇥ -->
            <ImageButton android:id="@+id/kb_arrows_toggle"                   style="@style/KbKey" ... /> <!-- move icon, highlights when open -->
            <ImageButton android:id="@+id/kb_enter"                           style="@style/KbPrimary" ... /> <!-- ↵ -->
        </LinearLayout>
    </HorizontalScrollView>

</LinearLayout>
```

Key differences from the current Svelte design:

- **Arrow subbar** is a native `LinearLayout` row with `visibility="gone"` by default; the arrows toggle button flips it to `VISIBLE`, mirroring the existing Svelte expand behaviour.
- **WebView padding updates automatically** via a persistent `GlobalLayoutListener` — no manual recalculation needed when the subbar opens or closes.
- `HorizontalScrollView` on the main bar handles overflow on narrow phones, same as the Svelte `overflow-x: auto` approach.
- Tab-switching buttons (1, 2, 3) can be hidden programmatically when screen width is below a threshold (`activity.resources.displayMetrics.widthPixels < threshold`).

---

## IPC Design: Sidebar Toggle

The sidebar toggle button (`Menu`) is the one piece that needs to affect Svelte UI state. Two options:

**Option A — JS event:** Kotlin triggers `"kb-sidebar-toggle"`, JS calls `toggleSidebar()`. Simple, same pattern as key input.

**Option B — Tauri command back to Rust:** More indirection than needed for a UI toggle.

Option A is the right choice.

---

## Implementation Steps

1. **Scaffold the plugin** using `pnpm tauri plugin new keybar` (Tauri CLI) or manually create `keybar.rs` + `KeybarPlugin.kt`.
2. **Create the XML layout** (`native_keybar.xml`) and button styles/drawables.
3. **Implement `KeybarPlugin.kt`** — view injection, modifier state, ANSI sequence construction, event triggering.
4. **Register the plugin** in `lib.rs`: `.plugin(keybar::init())`.
5. **Add the Kotlin plugin** to `app/build.gradle.kts` — it lives inside the existing Android project rather than as an external library since it needs access to app resources.
6. **Wire JS listener** in `app.svelte.ts` for `"kb-input"` and `"kb-sidebar-toggle"`.
7. **Remove Svelte keyboard** — in `+page.svelte`, remove the `{#if !isLinux}` `VirtualKeyboard` block; remove the `visualViewport` listeners and `keyboardHeight` state.
8. **Platform guard** — keep `VirtualKeyboard.svelte` for Linux desktop (it's used as a quick-access bar there too, though it's less critical).

---

## Open Questions

### 1. WebView padding vs. LayoutParams margin

`webView.setPadding(0, barHeight, 0, 0)` is the simplest approach but may cause `scrollback: 0` xterm.js content to be shifted rather than clipped. An alternative is to modify the WebView's `LayoutParams` to add a `topMargin` matching the bar height. The correct choice depends on how Tauri internally lays out the WebView within the `FrameLayout`.

Testing required: observe whether xterm.js's `FitAddon` correctly computes dimensions after the padding/margin adjustment.

### 2. Plugin location — external vs. in-tree

Tauri's `plugin new` command creates a separate Cargo workspace + Gradle module. For a plugin that's tightly coupled to the app (accessing `R.layout.*`, app-specific resources, and `MainActivity`'s context), it may be simpler to integrate the Kotlin directly into the existing Android project (`gen/android/`) rather than as an external plugin library. The trade-off: it's harder to share with other projects but avoids cross-module resource referencing.

**Recommendation**: keep it in-tree for now. Add `KeybarPlugin.kt` alongside `MainActivity.kt` and register it like the existing biometric bridge.

### 3. `gen/android/` regeneration

`gen/android/` is gitignored and regenerated by `tauri android init`. Any Kotlin files added there need to be either:

- Tracked via a patch/template mechanism, or
- Added to a `gen/android/.gitignore`-exempt list

The existing `MainActivity.kt`, `KeyStoreManager.kt`, etc., are committed and survive `tauri android init` because Tauri only regenerates the scaffolding files (build config, Rust bridge), not user-created source files. The same approach would work for `KeybarPlugin.kt`.

### 4. Modifier state synchronization

In the current design, modifier state (ctrl/alt/meta) lives in Svelte. If a physical keyboard event fires while a virtual modifier is active, the Svelte `onMount` keydown listener handles the combination.

With a native bar, modifier state lives in Kotlin. Physical keyboard events still go through the WebView's `keydown` listener. Two options:

- **Keep Svelte modifier listener**: On physical key press, Svelte checks modifier state by calling a Tauri command `get_mod_state()` → Kotlin returns current state. Adds latency.
- **Move physical key interception to Kotlin**: Override `onKeyDown` in `MainActivity` to intercept physical keypresses when any modifier is active. Kotlin constructs the escape sequence and sends it directly. The WebView never sees these key events.

The second option is cleaner but requires changes to `MainActivity.kt` beyond the plugin.

### 5. Sidebar toggle on Android

Currently the sidebar is toggled via the `Menu` button in the virtual keyboard bar. With a native bar, this button triggers a `"kb-sidebar-toggle"` event. The sidebar itself is a Svelte component, so no native equivalent is needed for it — just the toggle signal.

### 6. Zellij tab buttons (1, 2, 3)

These buttons call `appState.runZellijAction(sessionId, "go-to-tab N")` which goes through the Tauri command system. Options:

- **Keep in Svelte**: Tab buttons remain in a web overlay (small floating element) rather than the native bar, since they need session context that's managed in Svelte.
- **Via event**: Native buttons trigger `"kb-go-to-tab"` with `{ tab: N }`, JS listener calls `runZellijAction`.

The event approach keeps the bar fully native. Recommended.

---

## Alternative Approaches

### A. Keep Svelte keyboard but fix the bottom-bar visibility issue differently

The current approach mostly works. The `adjustResize` + Visual Viewport API combination is functional. The main remaining issue is cross-device consistency (some Android OEMs behave differently). This is the zero-cost path.

### B. Android System Accessibility Overlay

A floating overlay via `WindowManager.addView` with `TYPE_APPLICATION_OVERLAY` (requires `SYSTEM_ALERT_WINDOW` permission). This is how floating keyboard apps work. Not applicable here — requires an alarming permission grant and isn't scoped to the app.

### C. Capacitor-style plugin

Capacitor's Android plugin system provides a `getBridge().getWebView()` reference and wraps the WebView in a `CoordinatorLayout`. This gives more structured control over view hierarchy but would require adopting Capacitor, which conflicts with the existing Tauri architecture.

### D. Second Tauri Webview

Tauri v2 supports multiple webviews per window on desktop. On Android this is [not currently supported](https://github.com/tauri-apps/tauri/issues/8246). A second webview containing only the keyboard bar would otherwise be an elegant solution (keep the keyboard in Svelte, run it in an isolated webview positioned at the top).

### E. CSS-only fix with `env(safe-area-inset-*)` and `dvh`

The current approach already uses `100dvh` and the Visual Viewport API. A further refinement is to position the bar using `position: sticky` within a flex column layout rather than `position: fixed`. This avoids the Visual Viewport listener but still relies on `adjustResize` behaving correctly.

---

## Recommended Approach

**Phase 1 (low risk)**: Improve the current Svelte implementation using CSS `position: sticky` in a flex column, removing the Visual Viewport API listener. This is a low-effort fix that may resolve remaining positioning edge cases.

**Phase 2 (native bar)**: Implement the native top bar as an in-tree Kotlin plugin. The bar handles all key input natively, triggering `"kb-input"` events to Svelte. Modifier state lives in Kotlin. Physical key interception is handled by a `MainActivity.onKeyDown` override. The Svelte `VirtualKeyboard` component is removed from the mobile path entirely.

Phase 2 is the right long-term architecture. The main prerequisite is understanding how Tauri internally positions the WebView inside `android.R.id.content` to ensure the padding/margin approach doesn't break xterm.js's fit calculations.

---

## Testing

The plugin spans four distinct layers, each with a different testing approach.

```text
┌──────────────────────────────────────────────────────────┐
│  Layer          │  Framework          │  Speed / fidelity │
├──────────────────────────────────────────────────────────┤
│  ANSI logic     │  JUnit (JVM)        │  fast, no device  │
│  View hierarchy │  Robolectric / JVM  │  fast, ~real      │
│  Layout/padding │  Espresso (device)  │  slow, exact      │
│  JS handler     │  Vitest             │  fast, no device  │
│  End-to-end     │  manual checklist   │  device required  │
└──────────────────────────────────────────────────────────┘
```

---

### Layer 1 — ANSI sequence logic (JUnit, no Android)

The `sendArrow()` bitmask logic is pure computation, identical in structure to the existing TypeScript `modifiedArrow()` in `key-mapper.ts`. Extract it into a standalone Kotlin object so it can be tested without a device:

```kotlin
// KeybarSeqs.kt
object KeybarSeqs {
    fun modifiedArrow(letter: String, ctrl: Boolean, alt: Boolean, meta: Boolean): String {
        var mod = 0
        if (alt || meta) mod += 2
        if (ctrl)        mod += 4
        return if (mod == 0) "\u001b[$letter"
               else          "\u001b[1;${mod + 1}$letter"
    }
}
```

`KeybarPlugin.sendArrow()` delegates to `KeybarSeqs.modifiedArrow(letter, modCtrl, modAlt, modMeta)`.

```kotlin
// test/java/com/njr/zelland/KeybarSeqsTest.kt
class KeybarSeqsTest {

    @Test fun baseArrows_noModifiers() {
        assertEquals("\u001b[A", KeybarSeqs.modifiedArrow("A", ctrl=false, alt=false, meta=false))
        assertEquals("\u001b[B", KeybarSeqs.modifiedArrow("B", ctrl=false, alt=false, meta=false))
        assertEquals("\u001b[C", KeybarSeqs.modifiedArrow("C", ctrl=false, alt=false, meta=false))
        assertEquals("\u001b[D", KeybarSeqs.modifiedArrow("D", ctrl=false, alt=false, meta=false))
    }

    @Test fun ctrlArrow() {
        assertEquals("\u001b[1;5A", KeybarSeqs.modifiedArrow("A", ctrl=true,  alt=false, meta=false))
        assertEquals("\u001b[1;5B", KeybarSeqs.modifiedArrow("B", ctrl=true,  alt=false, meta=false))
        assertEquals("\u001b[1;5C", KeybarSeqs.modifiedArrow("C", ctrl=true,  alt=false, meta=false))
        assertEquals("\u001b[1;5D", KeybarSeqs.modifiedArrow("D", ctrl=true,  alt=false, meta=false))
    }

    @Test fun altArrow() {
        assertEquals("\u001b[1;3A", KeybarSeqs.modifiedArrow("A", ctrl=false, alt=true,  meta=false))
        assertEquals("\u001b[1;3D", KeybarSeqs.modifiedArrow("D", ctrl=false, alt=true,  meta=false))
    }

    @Test fun metaTreatedAsAlt() {
        assertEquals("\u001b[1;3A", KeybarSeqs.modifiedArrow("A", ctrl=false, alt=false, meta=true))
    }

    @Test fun ctrlAltArrow() {
        assertEquals("\u001b[1;7A", KeybarSeqs.modifiedArrow("A", ctrl=true,  alt=true,  meta=false))
        assertEquals("\u001b[1;7D", KeybarSeqs.modifiedArrow("D", ctrl=true,  alt=true,  meta=false))
    }
}
```

These 5 tests mirror `src/lib/utils/key-mapper.test.ts` exactly, acting as a cross-language contract: if the TS tests and Kotlin tests pass with the same inputs and outputs, the two implementations stay in sync.

**Gradle setup** — these are plain JUnit tests under `src/test/` (not `androidTest/`), so they run on the JVM with no emulator:

```kotlin
// app/build.gradle.kts
dependencies {
    testImplementation("junit:junit:4.13.2")
}
```

---

### Layer 2 — View hierarchy and state (Robolectric)

Robolectric runs Android framework code on the JVM, making it suitable for testing view visibility, modifier state toggles, and event emission without a physical device or emulator.

Add the dependency:

```kotlin
// app/build.gradle.kts
testImplementation("org.robolectric:robolectric:4.12.2")
testImplementation("androidx.test:core:1.5.0")
```

```kotlin
// test/java/com/njr/zelland/KeybarPluginTest.kt
@RunWith(RobolectricTestRunner::class)
class KeybarPluginTest {

    private lateinit var activity: AppCompatActivity
    private lateinit var plugin: KeybarPlugin

    @Before fun setUp() {
        activity = Robolectric.buildActivity(AppCompatActivity::class.java).create().get()
        plugin = KeybarPlugin(activity)
        // Simulate Tauri calling load() with a stub WebView
        plugin.load(WebView(activity))
    }

    // --- Arrow subbar toggle ---

    @Test fun arrowSubbar_hiddenByDefault() {
        val subbar = activity.findViewById<View>(R.id.kb_arrow_row)
        assertEquals(View.GONE, subbar.visibility)
    }

    @Test fun arrowsToggle_showsSubbar() {
        activity.findViewById<View>(R.id.kb_arrows_toggle).performClick()
        assertEquals(View.VISIBLE, activity.findViewById<View>(R.id.kb_arrow_row).visibility)
    }

    @Test fun arrowsToggle_hidesSubbarOnSecondPress() {
        val toggle = activity.findViewById<View>(R.id.kb_arrows_toggle)
        toggle.performClick()
        toggle.performClick()
        assertEquals(View.GONE, activity.findViewById<View>(R.id.kb_arrow_row).visibility)
    }

    // --- Modifier toggles ---

    @Test fun ctrlToggle_flipsBothWays() {
        val ctrlBtn = activity.findViewById<View>(R.id.kb_ctrl)
        ctrlBtn.performClick()
        assertTrue(plugin.modCtrl)   // requires modCtrl to be internal/package-visible for tests
        ctrlBtn.performClick()
        assertFalse(plugin.modCtrl)
    }

    @Test fun modifiersResetAfterKeySend() {
        activity.findViewById<View>(R.id.kb_ctrl).performClick()
        activity.findViewById<View>(R.id.kb_esc).performClick()
        assertFalse(plugin.modCtrl)
        assertFalse(plugin.modAlt)
        assertFalse(plugin.modMeta)
    }

    // --- Event payload ---

    @Test fun escButton_triggersEscSequence() {
        val events = mutableListOf<String>()
        plugin.onTrigger = { _, data -> events += data.getString("seq") ?: "" }

        activity.findViewById<View>(R.id.kb_esc).performClick()

        assertEquals(listOf("\u001b"), events)
    }

    @Test fun ctrlArrow_triggersModifiedSequence() {
        val events = mutableListOf<String>()
        plugin.onTrigger = { _, data -> events += data.getString("seq") ?: "" }

        activity.findViewById<View>(R.id.kb_ctrl).performClick()
        activity.findViewById<View>(R.id.kb_up).performClick()

        assertEquals(listOf("\u001b[1;5A"), events)
    }
}
```

**Note on `plugin.onTrigger`:** the Tauri `Plugin.trigger()` method isn't directly interceptable in unit tests. The cleanest approach is to extract event emission into an injectable lambda — `var onTrigger: (name: String, data: JSObject) -> Unit = { n, d -> trigger(n, d) }` — and replace it in tests. This is the standard test-seam pattern.

---

### Layer 3 — Layout and WebView padding (Espresso, device/emulator)

Robolectric doesn't execute `ViewTreeObserver` listeners accurately — actual view measurement requires a real rendering pass. These tests run as instrumented tests (`androidTest/`) on an emulator or device.

```kotlin
// androidTest/java/com/njr/zelland/KeybarLayoutTest.kt
@RunWith(AndroidJUnit4::class)
class KeybarLayoutTest {

    @get:Rule val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test fun nativeBar_isAddedAboveWebView() {
        activityRule.scenario.onActivity { activity ->
            val content = activity.window.decorView.findViewById<FrameLayout>(android.R.id.content)
            // Bar should be a child of the content frame
            val bar = content.findViewById<View>(R.id.kb_arrow_row)
            assertNotNull(bar)
        }
    }

    @Test fun webView_topPaddingEqualsBarHeight() {
        // Wait for layout to settle
        onView(withId(R.id.kb_esc)).check(matches(isDisplayed()))

        activityRule.scenario.onActivity { activity ->
            val content = activity.window.decorView.findViewById<FrameLayout>(android.R.id.content)
            val bar = content.findViewWithTag<View>("keybar_root") // tag set in setupKeybar()
            val webView = content.findViewWithTag<WebView>("tauri_webview") // may need tag injection

            assertEquals(bar.height, webView.paddingTop)
        }
    }

    @Test fun webView_paddingIncreasesWhenSubbarOpens() {
        // Record initial padding
        var initialPadding = 0
        activityRule.scenario.onActivity { activity ->
            val content = activity.window.decorView.findViewById<FrameLayout>(android.R.id.content)
            val wv = content.findViewWithTag<WebView>("tauri_webview")
            initialPadding = wv.paddingTop
        }

        // Open the arrow subbar
        onView(withId(R.id.kb_arrows_toggle)).perform(click())

        // Allow layout to settle
        onView(withId(R.id.kb_arrow_row)).check(matches(isDisplayed()))

        activityRule.scenario.onActivity { activity ->
            val content = activity.window.decorView.findViewById<FrameLayout>(android.R.id.content)
            val wv = content.findViewWithTag<WebView>("tauri_webview")
            assertTrue(wv.paddingTop > initialPadding)
        }
    }
}
```

**Prerequisite:** `setupKeybar()` must tag the root bar view (`bar.tag = "keybar_root"`) and identify the WebView. Finding the Tauri WebView by tag requires either Tauri exposing it or the plugin storing it in a known tag after `load()`.

---

### Layer 4 — TypeScript event handler (Vitest)

The JS side of the IPC is a thin handler that converts a sequence string to bytes and calls `writeInput`. Extract it for testing:

```typescript
// src/lib/utils/kb-input.ts
export function handleKbInput(
    seq: string,
    activeSessionId: string | null,
    writeInput: (id: string, bytes: number[]) => void,
): void {
    if (!activeSessionId) return;
    writeInput(activeSessionId, Array.from(new TextEncoder().encode(seq)));
}
```

```typescript
// src/lib/utils/kb-input.test.ts
import { describe, it, expect, vi } from 'vitest';
import { handleKbInput } from './kb-input';

describe('handleKbInput', () => {
    it('encodes ESC sequence to bytes', () => {
        const write = vi.fn();
        handleKbInput('\x1b', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x1b]);
    });

    it('encodes Enter', () => {
        const write = vi.fn();
        handleKbInput('\r', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x0d]);
    });

    it('encodes Ctrl+Up (\\x1b[1;5A)', () => {
        const write = vi.fn();
        handleKbInput('\x1b[1;5A', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x1b, 0x5b, 0x31, 0x3b, 0x35, 0x41]);
    });

    it('does nothing when no active session', () => {
        const write = vi.fn();
        handleKbInput('\r', null, write);
        expect(write).not.toHaveBeenCalled();
    });
});
```

Wire `handleKbInput` into the `listen("kb-input")` registration in `app.svelte.ts`:

```typescript
await listen<{ seq: string }>("kb-input", ({ payload }) => {
    handleKbInput(payload.seq, appState.activeSessionId, (id, bytes) => {
        appState.writeInput(id, bytes);
    });
});
```

---

### Layer 5 — Manual device checklist

Some behaviours require a physical device or emulator and can't be fully automated:

```text
[ ] Bar is visible at the top of the app on cold start
[ ] Bar remains visible when the system keyboard opens (type in a search box)
[ ] Bar remains visible when the system keyboard is dismissed
[ ] Arrow subbar appears below main bar when arrows toggle pressed
[ ] Arrow subbar hides when toggle pressed again
[ ] WebView content is not clipped behind the bar (scroll to top of terminal)
[ ] xterm.js FitAddon reports correct cols/rows after bar is added
[ ] xterm.js FitAddon reports updated cols/rows after subbar opens/closes
[ ] Ctrl+Down sends \x1b[1;5B to Zellij (jump between panes)
[ ] Alt+arrow sends \x1b[1;3X (word-jump in shell)
[ ] Modifier highlight (C/A/M button turns primary colour when active)
[ ] Modifier resets to inactive after key send
[ ] Haptic feedback fires on each button press
[ ] Menu button triggers sidebar open in Svelte
[ ] Tab buttons 1/2/3 change Zellij tab
[ ] Bar is NOT shown on Linux desktop build (platform guard active)
[ ] Rotate to landscape: bar still fits, HorizontalScrollView allows scroll
[ ] Narrow phone (<360dp): tab buttons hidden, remaining keys accessible
```

---

### Test file locations

```text
src-tauri/gen/android/app/
  src/
    test/java/com/njr/zelland/
      KeybarSeqsTest.kt          ← JUnit, runs on JVM (no device)
      KeybarPluginTest.kt        ← Robolectric, runs on JVM
    androidTest/java/com/njr/zelland/
      KeybarLayoutTest.kt        ← Espresso, requires emulator/device

src/lib/utils/
  kb-input.ts                    ← extracted handler
  kb-input.test.ts               ← Vitest
```

---

## References

- [Tauri v2 Mobile Plugin Development](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [Android AppBar / Toolbar setup](https://developer.android.com/develop/ui/views/components/appbar/setting-up)
- [Combining Tauri-generated Android code with native code (discussion)](https://github.com/tauri-apps/tauri/discussions/11444)
- [Tauri multiple WebViews / native rendering (issue)](https://github.com/tauri-apps/tauri/issues/8246)
- `src-tauri/src/intent.rs` — existing minimal Tauri plugin pattern in this project
- `src/lib/components/VirtualKeyboard.svelte` — current implementation to be replaced
- `src-tauri/gen/android/app/src/main/java/com/njr/zelland/MainActivity.kt` — Activity to be modified
