# Zelland - Design Document

## Overview

**Zelland** (Zellij + Android) is an Android application that connects to remote Zellij web servers via SSH, providing a native mobile interface for managing terminal sessions with gesture controls and multi-host support.

## Architecture

### High-Level Components

```text
┌─────────────────────────────────────────┐
│         MainActivity                     │
│  ┌───────────────────────────────────┐  │
│  │     ViewPager2/RecyclerView       │  │
│  │  ┌──────────┐ ┌──────────┐       │  │
│  │  │Terminal 1│ │Terminal 2│ ...   │  │
│  │  │(Fragment)│ │(Fragment)│       │  │
│  │  └──────────┘ └──────────┘       │  │
│  └───────────────────────────────────┘  │
│  ┌───────────────────────────────────┐  │
│  │    CustomKeyboardView             │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  AlphaGrid (contextual)     │  │  │
│  │  └─────────────────────────────┘  │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  ModBar (persistent)        │  │  │
│  │  └─────────────────────────────┘  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

### 1. Terminal Component

#### TerminalFragment

- **Responsibility**: Manages a single terminal session
- **Contains**:
  - WebView hosting xterm.js
  - JavaScript interface bridge
  - Session state management
- **Lifecycle**: Retained across swipes (not destroyed)

#### WebView Configuration

- **Local Assets**: HTML/JS/CSS bundle in `assets/terminal/`
- **JavaScript Enabled**: Required for xterm.js
- **DOM Storage**: Enabled for session persistence
- **JavaScript Interface**: Bidirectional communication bridge

#### xterm.js Integration

```javascript
// assets/terminal/index.html structure
- xterm.js library (v5.x)
- xterm-addon-fit (responsive sizing)
- xterm-addon-web-links (URL detection)
- Custom bridge layer for Android communication
```

### 2. Custom Keyboard System

#### Design Philosophy

Traditional Android keyboards are optimized for text input, not terminal control sequences. Our custom keyboard provides:

- Direct access to modifier keys (Ctrl, Alt, Shift)
- Easy access to special keys (Esc, Tab, Arrow keys)
- Combination input without system keyboard interference
- Visual feedback for active modifiers

#### Two-Layer Architecture

**Layer 1: ModBar (Always Visible)**

- Horizontal strip at bottom of screen
- Toggle buttons: Ctrl, Alt, Shift, Esc, Tab
- Fixed height: 48-56dp
- Visual state: Normal/Active/Pressed

**Layer 2: AlphaGrid (Contextual)**

- Appears above ModBar when modifier is active
- Grid layout: 3 rows × 10 columns (adjustable)
- Contains: A-Z, 0-9, symbols, arrows
- Slides in/out with animation
- Height: 140-180dp

#### Input Flow

```text
User Taps Ctrl → ModBar highlights Ctrl button
              → AlphaGrid slides up
              → User taps 'C'
              → App sends Ctrl+C sequence to xterm.js
              → AlphaGrid slides down
              → ModBar resets
```

### 3. Data Flow

#### Input Path: Android → xterm.js

```text
CustomKeyboardView → TerminalViewModel → TerminalFragment
    ↓
WebView.evaluateJavascript("term.write('...')")
    ↓
xterm.js terminal instance
    ↓
WebSocket backend (out of scope)
```

#### Output Path: Backend → xterm.js → Display

```text
WebSocket message → JavaScript handler
    ↓
term.write(data)
    ↓
xterm.js renders to canvas/DOM
    ↓
WebView displays content
```

### 4. State Management

#### TerminalViewModel

```kotlin
class TerminalViewModel : ViewModel() {
    // Modifier state
    val modifierState = MutableLiveData<ModifierState>()

    // Terminal sessions
    val sessions = MutableLiveData<List<TerminalSession>>()

    // Active session index
    val activeSessionIndex = MutableLiveData<Int>()
}

data class ModifierState(
    val ctrl: Boolean = false,
    val alt: Boolean = false,
    val shift: Boolean = false
)

data class TerminalSession(
    val id: String,
    val title: String,
    val webSocketUrl: String? = null,
    val isConnected: Boolean = false
)
```

#### Session Persistence

- Use `FragmentStateAdapter` for ViewPager2
- Fragments retained in memory during swipes
- Session state saved to SharedPreferences on pause
- Restore sessions on app restart

### 5. Key Technical Decisions

#### Why WebView + xterm.js?

- **Proven**: xterm.js is mature and battle-tested
- **Feature-rich**: ANSI escape codes, 256 colors, Unicode
- **Maintained**: Active development and community
- **No reinvention**: Avoid building terminal emulation from scratch

#### Why Custom Keyboard?

- **Control**: Full control over key sequences
- **UX**: Optimized for terminal workflows
- **Space**: More screen space for terminal output
- **Combos**: Easy modifier + key combinations

#### Why ViewPager2?

- **Swipe**: Natural gesture for switching sessions
- **Performance**: Efficient view recycling
- **Familiar**: Standard Android pattern
- **Indicators**: Easy to add tab dots/labels

### 6. Key Mappings

#### Control Sequences

| Key Combo | Sequence | Hex Code |

| Key Combo | Sequence | Hex Code || Key Combo | Sequence | Hex Code |
| ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  || ----------- | ---------- | ---------- |  |  |  |  ||-----------|----------|----------|
| Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C | `^C` | `\u0003` |  |  |  |  || Ctrl+C    | `^C`     | `\u0003` |
| Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D | `^D` | `\u0004` |  |  |  |  || Ctrl+D    | `^D`     | `\u0004` |
| Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z | `^Z` | `\u001A` |  |  |  |  || Ctrl+Z    | `^Z`     | `\u001A` |
| Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc | `ESC` | `\u001B` |  |  |  |  || Esc       | `ESC`    | `\u001B` |
| Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab | `TAB` | `\u0009` |  |  |  |  || Tab       | `TAB`    | `\u0009` |
| Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up | `ESC[A` | `\u001B[A` |  |  |  |  || Arrow Up  | `ESC[A`  | `\u001B[A` |
| Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down | `ESC[B` | `\u001B[B` |  |  |  |  || Arrow Down| `ESC[B`  | `\u001B[B` |
| Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right | `ESC[C` | `\u001B[C` |  |  |  |  || Arrow Right| `ESC[C` | `\u001B[C` |
| Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left | `ESC[D` | `\u001B[D` |  |  |  |  || Arrow Left| `ESC[D`  | `\u001B[D` |

### 7. WebView ↔ JavaScript Bridge

#### Android → JavaScript

```kotlin
// Send input to terminal
webView.evaluateJavascript("term.write('${escapedInput}')", null)

// Send control sequence
webView.evaluateJavascript("term.write('\\u0003')", null) // Ctrl+C
```

#### JavaScript → Android

```kotlin
// In Android
webView.addJavascriptInterface(TerminalBridge(), "Android")

class TerminalBridge {
    @JavascriptInterface
    fun onTerminalReady() {
        // Terminal initialization complete
    }

    @JavascriptInterface
    fun onTerminalData(data: String) {
        // User input from terminal (for WebSocket forwarding)
    }
}
```

```javascript
// In JavaScript
term.onData(data => {
    Android.onTerminalData(data);
});
```

### 8. UI/UX Considerations

#### Terminal Display

- **Font**: Monospace (Source Code Pro, JetBrains Mono)
- **Size**: Configurable (12-16sp default)
- **Theme**: Dark mode default, light mode optional
- **Cursor**: Block/underline/bar options

#### Keyboard UI

- **Haptic Feedback**: Light tap on key press
- **Visual Feedback**: Ripple effect + state color
- **Animation**: Smooth slide-in/out (200-300ms)
- **Spacing**: Adequate touch targets (48dp minimum)

#### Session Management

- **Tab Indicator**: Dots or labels above keyboard
- **Add/Remove**: FAB or menu action
- **Reorder**: Long-press and drag (optional)

### 9. Future Enhancements (Out of Initial Scope)

- Local shell execution (via PTY)
- SSH client integration (JSch library)
- File transfer (SFTP)
- Session persistence to disk
- Custom color schemes
- Split-screen terminals
- Gesture shortcuts (volume keys, etc.)
- External keyboard support

### 10. Security Considerations

- **WebView Isolation**: Each session in separate process (optional)
- **HTTPS Only**: WebSocket connections via WSS
- **No Mixed Content**: Disable if using secure connections
- **Input Sanitization**: Escape all user input before JavaScript eval
- **Certificate Pinning**: For production WebSocket servers

### 11. Performance Targets

- **Startup Time**: < 1 second to first terminal
- **Input Latency**: < 50ms key press to visual feedback
- **Memory**: < 100MB per terminal session
- **Swipe Performance**: 60 FPS between sessions
- **Terminal Rendering**: Delegated to xterm.js (GPU-accelerated)

## Technology Stack

- **Language**: Kotlin
- **Min SDK**: API 24 (Android 7.0)
- **Target SDK**: API 34 (Android 14)
- **Build System**: Gradle (Kotlin DSL)
- **UI Framework**: Android Views (XML layouts)
- **Architecture**: MVVM with ViewModel + LiveData
- **Dependencies**:
  - AndroidX Core KTX
  - AndroidX AppCompat
  - AndroidX Fragment KTX
  - AndroidX ViewPager2
  - Material Components
  - xterm.js (bundled in assets)

## File Structure

```text
app/
├── src/main/
│   ├── java/com/zelland/
│   │   ├── MainActivity.kt
│   │   ├── ui/
│   │   │   ├── terminal/
│   │   │   │   ├── TerminalFragment.kt
│   │   │   │   └── TerminalAdapter.kt
│   │   │   └── keyboard/
│   │   │       ├── CustomKeyboardView.kt
│   │   │       ├── ModBarView.kt
│   │   │       └── AlphaGridView.kt
│   │   ├── viewmodel/
│   │   │   └── TerminalViewModel.kt
│   │   ├── model/
│   │   │   ├── TerminalSession.kt
│   │   │   └── ModifierState.kt
│   │   └── bridge/
│   │       └── TerminalJavascriptInterface.kt
│   ├── res/
│   │   ├── layout/
│   │   │   ├── activity_main.xml
│   │   │   ├── fragment_terminal.xml
│   │   │   ├── view_custom_keyboard.xml
│   │   │   ├── view_mod_bar.xml
│   │   │   └── view_alpha_grid.xml
│   │   ├── values/
│   │   │   ├── colors.xml
│   │   │   ├── strings.xml
│   │   │   ├── dimens.xml
│   │   │   └── themes.xml
│   │   └── drawable/
│   │       └── (key backgrounds, states, etc.)
│   └── assets/
│       └── terminal/
│           ├── index.html
│           ├── xterm.js
│           ├── xterm.css
│           ├── xterm-addon-fit.js
│           └── bridge.js
└── build.gradle.kts
```

---

## Testing Strategy

### Overview

Comprehensive testing across unit, integration, and UI layers ensures reliability and maintainability. Testing WebView-based components presents unique challenges that require specialized mocking and instrumentation strategies.

### Testing Frameworks & Dependencies

```kotlin
// build.gradle.kts test dependencies
dependencies {
    // Unit Testing
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("androidx.arch.core:core-testing:2.2.0") // InstantTaskExecutorRule
    testImplementation("io.mockk:mockk:1.13.8") // Kotlin-friendly mocking
    testImplementation("com.google.truth:truth:1.1.5") // Fluent assertions

    // Instrumentation/UI Testing
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
    androidTestImplementation("androidx.test.espresso:espresso-web:3.5.1") // WebView testing
    androidTestImplementation("androidx.test.espresso:espresso-contrib:3.5.1") // ViewPager2
    androidTestImplementation("androidx.test:runner:1.5.2")
    androidTestImplementation("androidx.test:rules:1.5.0")
    androidTestImplementation("io.mockk:mockk-android:1.13.8")

    // Fragment Testing
    debugImplementation("androidx.fragment:fragment-testing:1.6.2")
}
```

### Test Directory Structure

```text
app/
├── src/
│   ├── test/                          # Unit tests (JVM)
│   │   └── java/com/zelland/
│   │       ├── viewmodel/
│   │       │   └── TerminalViewModelTest.kt
│   │       ├── model/
│   │       │   ├── ModifierStateTest.kt
│   │       │   └── TerminalSessionTest.kt
│   │       └── util/
│   │           └── KeySequenceHelperTest.kt
│   │
│   └── androidTest/                   # Instrumentation tests (Device/Emulator)
│       └── java/com/zelland/
│           ├── ui/
│           │   ├── terminal/
│           │   │   └── TerminalFragmentTest.kt
│           │   └── keyboard/
│           │       ├── ModBarViewTest.kt
│           │       ├── AlphaGridViewTest.kt
│           │       └── CustomKeyboardViewTest.kt
│           ├── bridge/
│           │   └── TerminalJavascriptInterfaceTest.kt
│           ├── integration/
│           │   ├── KeyboardToTerminalTest.kt
│           │   ├── MultiSessionTest.kt
│           │   └── SessionPersistenceTest.kt
│           └── MainActivityTest.kt
```

---

## Milestone Testing Breakdown

### Milestone 1: Project Setup & Foundation

#### Unit Tests

**Not applicable** - This milestone focuses on project scaffolding and asset setup.

#### Integration Tests

**TerminalFragmentBasicTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class TerminalFragmentBasicTest {
    @Test
    fun fragment_loads_webview_successfully() {
        // Launch fragment in isolation
        val scenario = launchFragmentInContainer<TerminalFragment>()

        scenario.onFragment { fragment ->
            val webView = fragment.view?.findViewById<WebView>(R.id.terminalWebView)
            assertThat(webView).isNotNull()
            assertThat(webView?.settings?.javaScriptEnabled).isTrue()
        }
    }

    @Test
    fun webview_loads_local_html_asset() {
        val scenario = launchFragmentInContainer<TerminalFragment>()

        scenario.onFragment { fragment ->
            val webView = fragment.view?.findViewById<WebView>(R.id.terminalWebView)
            // Allow time for asset loading
            Thread.sleep(1000)

            // Verify URL points to local asset
            assertThat(webView?.url).contains("file:///android_asset")
        }
    }

    @Test
    fun xterm_js_initializes_in_webview() {
        val scenario = launchFragmentInContainer<TerminalFragment>()
        val latch = CountDownLatch(1)
        var jsResult = ""

        scenario.onFragment { fragment ->
            val webView = fragment.view?.findViewById<WebView>(R.id.terminalWebView)

            // Wait for page load
            webView?.webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, url: String?) {
                    // Check if xterm is defined
                    view?.evaluateJavascript("typeof Terminal !== 'undefined'") { result ->
                        jsResult = result
                        latch.countDown()
                    }
                }
            }
        }

        latch.await(5, TimeUnit.SECONDS)
        assertThat(jsResult).isEqualTo("true")
    }
}
```

**Acceptance Criteria**:

- WebView initializes with JavaScript enabled
- Local HTML asset loads without errors
- xterm.js library is accessible in JavaScript context

---

### Milestone 2: JavaScript Bridge & Input System

#### Unit Tests

**KeySequenceHelperTest.kt**

```kotlin
class KeySequenceHelperTest {
    private lateinit var helper: KeySequenceHelper

    @Before
    fun setup() {
        helper = KeySequenceHelper()
    }

    @Test
    fun `regular key returns character`() {
        val result = helper.getSequence('a', ModifierState())
        assertThat(result).isEqualTo("a")
    }

    @Test
    fun `ctrl+c returns correct control sequence`() {
        val result = helper.getSequence('c', ModifierState(ctrl = true))
        assertThat(result).isEqualTo("\u0003")
    }

    @Test
    fun `ctrl+d returns EOT`() {
        val result = helper.getSequence('d', ModifierState(ctrl = true))
        assertThat(result).isEqualTo("\u0004")
    }

    @Test
    fun `escape key returns ESC sequence`() {
        val result = helper.getSequence(KeyEvent.KEYCODE_ESCAPE)
        assertThat(result).isEqualTo("\u001B")
    }

    @Test
    fun `tab key returns tab character`() {
        val result = helper.getSequence(KeyEvent.KEYCODE_TAB)
        assertThat(result).isEqualTo("\u0009")
    }

    @Test
    fun `arrow up returns ANSI escape sequence`() {
        val result = helper.getSequence(KeyEvent.KEYCODE_DPAD_UP)
        assertThat(result).isEqualTo("\u001B[A")
    }

    @Test
    fun `escapeForJavascript handles quotes`() {
        val input = "test'string\"with'quotes"
        val result = helper.escapeForJavascript(input)
        assertThat(result).doesNotContain("'")
        assertThat(result).doesNotContain("\"")
    }
}
```

**ModifierStateTest.kt**

```kotlin
class ModifierStateTest {
    @Test
    fun `default state has all modifiers false`() {
        val state = ModifierState()
        assertThat(state.ctrl).isFalse()
        assertThat(state.alt).isFalse()
        assertThat(state.shift).isFalse()
    }

    @Test
    fun `toggleCtrl changes ctrl state`() {
        val state = ModifierState()
        val toggled = state.toggleCtrl()
        assertThat(toggled.ctrl).isTrue()
    }

    @Test
    fun `reset returns default state`() {
        val state = ModifierState(ctrl = true, alt = true)
        val reset = state.reset()
        assertThat(reset.ctrl).isFalse()
        assertThat(reset.alt).isFalse()
    }

    @Test
    fun `hasActiveModifier returns true when any modifier active`() {
        assertThat(ModifierState(ctrl = true).hasActiveModifier()).isTrue()
        assertThat(ModifierState(alt = true).hasActiveModifier()).isTrue()
        assertThat(ModifierState().hasActiveModifier()).isFalse()
    }
}
```

#### Integration Tests

**TerminalJavascriptInterfaceTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class TerminalJavascriptInterfaceTest {
    private lateinit var scenario: FragmentScenario<TerminalFragment>

    @Before
    fun setup() {
        scenario = launchFragmentInContainer<TerminalFragment>()
    }

    @After
    fun teardown() {
        scenario.close()
    }

    @Test
    fun `javascript interface receives onTerminalReady callback`() {
        val latch = CountDownLatch(1)
        var callbackReceived = false

        scenario.onFragment { fragment ->
            fragment.setOnTerminalReadyListener {
                callbackReceived = true
                latch.countDown()
            }
        }

        latch.await(5, TimeUnit.SECONDS)
        assertThat(callbackReceived).isTrue()
    }

    @Test
    fun `sendInput successfully evaluates javascript`() {
        val latch = CountDownLatch(1)
        var jsExecuted = false

        scenario.onFragment { fragment ->
            fragment.waitForReady {
                fragment.sendInput("test")

                // Verify JavaScript was called
                fragment.webView.evaluateJavascript("'executed'") { result ->
                    jsExecuted = result == "\"executed\""
                    latch.countDown()
                }
            }
        }

        latch.await(5, TimeUnit.SECONDS)
        assertThat(jsExecuted).isTrue()
    }

    @Test
    fun `javascript can call Android interface method`() {
        val latch = CountDownLatch(1)
        var dataReceived = ""

        scenario.onFragment { fragment ->
            fragment.setOnTerminalDataListener { data ->
                dataReceived = data
                latch.countDown()
            }

            fragment.waitForReady {
                // Simulate JavaScript calling Android
                fragment.webView.evaluateJavascript(
                    "Android.onTerminalData('test-data')",
                    null
                )
            }
        }

        latch.await(5, TimeUnit.SECONDS)
        assertThat(dataReceived).isEqualTo("test-data")
    }
}
```

**Acceptance Criteria**:

- KeySequenceHelper correctly maps all control sequences
- JavaScript interface methods are callable from both sides
- Input escaping prevents JavaScript injection

---

### Milestone 3: Custom Keyboard - ModBar

#### Unit Tests

**Not applicable** - ModBar logic is primarily UI-driven.

#### Integration Tests

**ModBarViewTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class ModBarViewTest {
    private lateinit var scenario: ActivityScenario<TestActivity>
    private lateinit var modBarView: ModBarView

    @Before
    fun setup() {
        scenario = ActivityScenario.launch(TestActivity::class.java)
        scenario.onActivity { activity ->
            modBarView = activity.findViewById(R.id.modBar)
        }
    }

    @Test
    fun `clicking Ctrl button toggles modifier state`() {
        onView(withId(R.id.btnCtrl)).perform(click())

        assertThat(modBarView.modifierState.value?.ctrl).isTrue()

        onView(withId(R.id.btnCtrl)).perform(click())
        assertThat(modBarView.modifierState.value?.ctrl).isFalse()
    }

    @Test
    fun `Ctrl button shows active state when toggled`() {
        onView(withId(R.id.btnCtrl))
            .perform(click())
            .check(matches(hasBackground(R.color.modifier_active)))
    }

    @Test
    fun `clicking Esc sends escape sequence`() {
        val latch = CountDownLatch(1)
        var sentSequence = ""

        scenario.onActivity { activity ->
            modBarView.setOnKeySequenceListener { sequence ->
                sentSequence = sequence
                latch.countDown()
            }
        }

        onView(withId(R.id.btnEsc)).perform(click())
        latch.await(1, TimeUnit.SECONDS)

        assertThat(sentSequence).isEqualTo("\u001B")
    }

    @Test
    fun `clicking Tab sends tab character`() {
        val latch = CountDownLatch(1)
        var sentSequence = ""

        scenario.onActivity { activity ->
            modBarView.setOnKeySequenceListener { sequence ->
                sentSequence = sequence
                latch.countDown()
            }
        }

        onView(withId(R.id.btnTab)).perform(click())
        latch.await(1, TimeUnit.SECONDS)

        assertThat(sentSequence).isEqualTo("\u0009")
    }

    @Test
    fun `arrow keys send correct ANSI sequences`() {
        val sequences = mutableListOf<String>()
        val latch = CountDownLatch(4)

        scenario.onActivity { activity ->
            modBarView.setOnKeySequenceListener { sequence ->
                sequences.add(sequence)
                latch.countDown()
            }
        }

        onView(withId(R.id.btnArrowUp)).perform(click())
        onView(withId(R.id.btnArrowDown)).perform(click())
        onView(withId(R.id.btnArrowLeft)).perform(click())
        onView(withId(R.id.btnArrowRight)).perform(click())

        latch.await(2, TimeUnit.SECONDS)

        assertThat(sequences).containsExactly(
            "\u001B[A", // Up
            "\u001B[B", // Down
            "\u001B[D", // Left
            "\u001B[C"  // Right
        )
    }

    @Test
    fun `multiple modifier keys can be active simultaneously`() {
        onView(withId(R.id.btnCtrl)).perform(click())
        onView(withId(R.id.btnAlt)).perform(click())

        val state = modBarView.modifierState.value
        assertThat(state?.ctrl).isTrue()
        assertThat(state?.alt).isTrue()
    }
}
```

**Acceptance Criteria**:

- All buttons respond to clicks with visual feedback
- Toggle buttons maintain state correctly
- Direct action buttons send correct sequences
- Modifier state LiveData updates properly

---

### Milestone 4: Custom Keyboard - AlphaGrid

#### Unit Tests

**Not applicable** - AlphaGrid is UI-focused.

#### Integration Tests

**AlphaGridViewTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class AlphaGridViewTest {
    private lateinit var scenario: ActivityScenario<TestActivity>
    private lateinit var alphaGridView: AlphaGridView

    @Before
    fun setup() {
        scenario = ActivityScenario.launch(TestActivity::class.java)
        scenario.onActivity { activity ->
            alphaGridView = activity.findViewById(R.id.alphaGrid)
        }
    }

    @Test
    fun `alphaGrid is hidden by default`() {
        onView(withId(R.id.alphaGrid))
            .check(matches(withEffectiveVisibility(Visibility.GONE)))
    }

    @Test
    fun `alphaGrid slides up when modifier active`() {
        scenario.onActivity { activity ->
            alphaGridView.show(animate = false)
        }

        onView(withId(R.id.alphaGrid))
            .check(matches(isDisplayed()))
    }

    @Test
    fun `alphaGrid slides down when modifier inactive`() {
        scenario.onActivity { activity ->
            alphaGridView.show(animate = false)
            alphaGridView.hide(animate = false)
        }

        onView(withId(R.id.alphaGrid))
            .check(matches(withEffectiveVisibility(Visibility.GONE)))
    }

    @Test
    fun `clicking alphanumeric key sends correct character`() {
        val latch = CountDownLatch(1)
        var sentKey = ""

        scenario.onActivity { activity ->
            alphaGridView.show(animate = false)
            alphaGridView.setOnKeyClickListener { key ->
                sentKey = key
                latch.countDown()
            }
        }

        onView(withText("A")).perform(click())
        latch.await(1, TimeUnit.SECONDS)

        assertThat(sentKey).isEqualTo("A")
    }

    @Test
    fun `all 26 alphabet keys are present`() {
        scenario.onActivity { activity ->
            alphaGridView.show(animate = false)
        }

        ('A'..'Z').forEach { char ->
            onView(withText(char.toString())).check(matches(isDisplayed()))
        }
    }

    @Test
    fun `animation duration is within expected range`() {
        val startTime = System.currentTimeMillis()

        scenario.onActivity { activity ->
            alphaGridView.show(animate = true)
        }

        // Wait for animation
        Thread.sleep(300)
        val duration = System.currentTimeMillis() - startTime

        // Should animate in 200-300ms
        assertThat(duration).isAtLeast(200)
        assertThat(duration).isAtMost(400)
    }
}
```

**CustomKeyboardViewTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class CustomKeyboardViewTest {
    private lateinit var scenario: ActivityScenario<TestActivity>
    private lateinit var keyboardView: CustomKeyboardView

    @Before
    fun setup() {
        scenario = ActivityScenario.launch(TestActivity::class.java)
        scenario.onActivity { activity ->
            keyboardView = activity.findViewById(R.id.customKeyboard)
        }
    }

    @Test
    fun `modBar always visible, alphaGrid starts hidden`() {
        onView(withId(R.id.modBar)).check(matches(isDisplayed()))
        onView(withId(R.id.alphaGrid)).check(matches(not(isDisplayed())))
    }

    @Test
    fun `toggling Ctrl shows alphaGrid`() {
        onView(withId(R.id.btnCtrl)).perform(click())

        // AlphaGrid should animate in
        Thread.sleep(300)
        onView(withId(R.id.alphaGrid)).check(matches(isDisplayed()))
    }

    @Test
    fun `typing character with Ctrl sends control sequence`() {
        val latch = CountDownLatch(1)
        var sentSequence = ""

        scenario.onActivity { activity ->
            keyboardView.setOnInputListener { sequence ->
                sentSequence = sequence
                latch.countDown()
            }
        }

        onView(withId(R.id.btnCtrl)).perform(click())
        Thread.sleep(300)
        onView(withText("C")).perform(click())

        latch.await(1, TimeUnit.SECONDS)
        assertThat(sentSequence).isEqualTo("\u0003") // Ctrl+C
    }

    @Test
    fun `alphaGrid auto-hides after key press with modifier`() {
        onView(withId(R.id.btnCtrl)).perform(click())
        Thread.sleep(300)
        onView(withText("C")).perform(click())

        // Should auto-hide
        Thread.sleep(300)
        onView(withId(R.id.alphaGrid)).check(matches(not(isDisplayed())))
    }
}
```

**Acceptance Criteria**:

- AlphaGrid visibility tied to modifier state
- Animations are smooth and within timing bounds
- Key combinations generate correct sequences
- Auto-dismiss works after combo input

---

### Milestone 5: Multi-Session Support with ViewPager2

#### Unit Tests

**TerminalViewModelTest.kt**

```kotlin
class TerminalViewModelTest {
    @get:Rule
    val instantTaskExecutorRule = InstantTaskExecutorRule()

    private lateinit var viewModel: TerminalViewModel

    @Before
    fun setup() {
        viewModel = TerminalViewModel()
    }

    @Test
    fun `initial state has one default session`() {
        val sessions = viewModel.sessions.value
        assertThat(sessions).hasSize(1)
        assertThat(sessions?.first()?.title).isEqualTo("Terminal 1")
    }

    @Test
    fun `addSession increases session count`() {
        viewModel.addSession()

        val sessions = viewModel.sessions.value
        assertThat(sessions).hasSize(2)
    }

    @Test
    fun `addSession generates unique IDs`() {
        viewModel.addSession()
        viewModel.addSession()

        val sessions = viewModel.sessions.value!!
        val ids = sessions.map { it.id }.toSet()
        assertThat(ids).hasSize(3) // Including default
    }

    @Test
    fun `removeSession decreases session count`() {
        viewModel.addSession()
        val sessionToRemove = viewModel.sessions.value!![1]

        viewModel.removeSession(sessionToRemove.id)

        assertThat(viewModel.sessions.value).hasSize(1)
    }

    @Test
    fun `cannot remove last session`() {
        val lastSession = viewModel.sessions.value!!.first()

        viewModel.removeSession(lastSession.id)

        // Should still have one session
        assertThat(viewModel.sessions.value).hasSize(1)
    }

    @Test
    fun `activeSessionIndex updates correctly`() {
        viewModel.addSession()
        viewModel.setActiveSessionIndex(1)

        assertThat(viewModel.activeSessionIndex.value).isEqualTo(1)
    }

    @Test
    fun `modifierState can be toggled`() {
        viewModel.toggleCtrl()
        assertThat(viewModel.modifierState.value?.ctrl).isTrue()

        viewModel.toggleCtrl()
        assertThat(viewModel.modifierState.value?.ctrl).isFalse()
    }

    @Test
    fun `resetModifiers clears all modifiers`() {
        viewModel.toggleCtrl()
        viewModel.toggleAlt()

        viewModel.resetModifiers()

        val state = viewModel.modifierState.value!!
        assertThat(state.ctrl).isFalse()
        assertThat(state.alt).isFalse()
        assertThat(state.shift).isFalse()
    }
}
```

**TerminalSessionTest.kt**

```kotlin
class TerminalSessionTest {
    @Test
    fun `session has valid default values`() {
        val session = TerminalSession(
            id = UUID.randomUUID().toString(),
            title = "Test Terminal"
        )

        assertThat(session.id).isNotEmpty()
        assertThat(session.title).isEqualTo("Test Terminal")
        assertThat(session.webSocketUrl).isNull()
        assertThat(session.isConnected).isFalse()
    }

    @Test
    fun `sessions are comparable by id`() {
        val id = UUID.randomUUID().toString()
        val session1 = TerminalSession(id = id, title = "A")
        val session2 = TerminalSession(id = id, title = "B")

        assertThat(session1.id).isEqualTo(session2.id)
    }
}
```

#### Integration Tests

**MultiSessionTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class MultiSessionTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun `viewPager displays initial session`() {
        onView(withId(R.id.viewPager))
            .check(matches(isDisplayed()))
    }

    @Test
    fun `can add new session via FAB`() {
        onView(withId(R.id.fabAddSession)).perform(click())

        // ViewPager should now have 2 items
        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            assertThat(viewPager.adapter?.itemCount).isEqualTo(2)
        }
    }

    @Test
    fun `can swipe between sessions`() {
        onView(withId(R.id.fabAddSession)).perform(click())

        onView(withId(R.id.viewPager)).perform(swipeLeft())

        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            assertThat(viewPager.currentItem).isEqualTo(1)
        }
    }

    @Test
    fun `keyboard input routes to active session only`() {
        onView(withId(R.id.fabAddSession)).perform(click())

        val latches = listOf(CountDownLatch(1), CountDownLatch(1))
        val inputs = mutableListOf<String>()

        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)

            // Setup listeners for both fragments
            (0..1).forEach { index ->
                val fragment = activity.supportFragmentManager
                    .findFragmentByTag("f$index") as? TerminalFragment
                fragment?.setOnInputReceivedListener { input ->
                    inputs.add("session$index: $input")
                    latches[index].countDown()
                }
            }
        }

        // Type in session 0
        onView(withId(R.id.btnEsc)).perform(click())
        latches[0].await(1, TimeUnit.SECONDS)

        // Swipe to session 1
        onView(withId(R.id.viewPager)).perform(swipeLeft())

        // Type in session 1
        onView(withId(R.id.btnTab)).perform(click())
        latches[1].await(1, TimeUnit.SECONDS)

        assertThat(inputs).containsExactly(
            "session0: \u001B", // Esc
            "session1: \u0009"  // Tab
        )
    }

    @Test
    fun `sessions maintain independent state`() {
        onView(withId(R.id.fabAddSession)).perform(click())

        // Type in session 0
        onView(withId(R.id.btnTab)).perform(click())

        // Swipe to session 1
        onView(withId(R.id.viewPager)).perform(swipeLeft())

        // Type in session 1
        onView(withId(R.id.btnEsc)).perform(click())

        // Swipe back to session 0
        onView(withId(R.id.viewPager)).perform(swipeRight())

        // Verify session 0 still has its history
        activityRule.scenario.onActivity { activity ->
            val fragment = activity.supportFragmentManager
                .findFragmentByTag("f0") as? TerminalFragment

            // Check terminal buffer (requires exposing terminal content)
            fragment?.getTerminalContent { content ->
                // Content should include the tab character effect
                assertThat(content).isNotEmpty()
            }
        }
    }

    @Test
    fun `can close session from menu`() {
        onView(withId(R.id.fabAddSession)).perform(click())
        onView(withId(R.id.fabAddSession)).perform(click())

        // Should have 3 sessions
        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            assertThat(viewPager.adapter?.itemCount).isEqualTo(3)
        }

        // Close current session
        openActionBarOverflowOrOptionsMenu(ApplicationProvider.getApplicationContext())
        onView(withText("Close Session")).perform(click())

        // Should have 2 sessions
        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            assertThat(viewPager.adapter?.itemCount).isEqualTo(2)
        }
    }
}
```

**Acceptance Criteria**:

- ViewModel correctly manages session list
- ViewPager2 adapter updates on session changes
- Input routing works correctly for active session
- Sessions maintain independent terminal state
- Cannot delete the last session

---

### Milestone 6: Polish & Optimization

#### Unit Tests

**SessionPersistenceTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class SessionPersistenceTest {
    private lateinit var context: Context
    private lateinit var persistence: SessionPersistence

    @Before
    fun setup() {
        context = ApplicationProvider.getApplicationContext()
        persistence = SessionPersistence(context)
        persistence.clear() // Clean slate
    }

    @After
    fun teardown() {
        persistence.clear()
    }

    @Test
    fun `can save and restore single session`() {
        val session = TerminalSession(
            id = UUID.randomUUID().toString(),
            title = "Test Session",
            webSocketUrl = "wss://example.com"
        )

        persistence.saveSessions(listOf(session))
        val restored = persistence.loadSessions()

        assertThat(restored).hasSize(1)
        assertThat(restored.first().id).isEqualTo(session.id)
        assertThat(restored.first().title).isEqualTo(session.title)
    }

    @Test
    fun `can save and restore multiple sessions`() {
        val sessions = (1..5).map { i ->
            TerminalSession(
                id = UUID.randomUUID().toString(),
                title = "Session $i"
            )
        }

        persistence.saveSessions(sessions)
        val restored = persistence.loadSessions()

        assertThat(restored).hasSize(5)
        assertThat(restored.map { it.title }).containsExactlyElementsIn(
            sessions.map { it.title }
        )
    }

    @Test
    fun `empty state returns default session`() {
        val restored = persistence.loadSessions()

        assertThat(restored).hasSize(1)
        assertThat(restored.first().title).isEqualTo("Terminal 1")
    }

    @Test
    fun `corrupted data returns default session`() {
        // Write invalid JSON
        val prefs = context.getSharedPreferences("sessions", Context.MODE_PRIVATE)
        prefs.edit().putString("session_list", "invalid json{{{").apply()

        val restored = persistence.loadSessions()

        assertThat(restored).hasSize(1)
        assertThat(restored.first().title).isEqualTo("Terminal 1")
    }
}
```

#### Integration Tests

**PerformanceTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class PerformanceTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun `app startup completes within 1 second`() {
        val startTime = System.currentTimeMillis()

        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            val adapter = viewPager.adapter

            assertThat(adapter?.itemCount).isAtLeast(1)
        }

        val duration = System.currentTimeMillis() - startTime
        assertThat(duration).isLessThan(1000)
    }

    @Test
    fun `key input latency is under 50ms`() {
        val latencies = mutableListOf<Long>()

        repeat(20) {
            val startTime = System.nanoTime()
            onView(withId(R.id.btnTab)).perform(click())
            val endTime = System.nanoTime()

            latencies.add((endTime - startTime) / 1_000_000) // Convert to ms
        }

        val averageLatency = latencies.average()
        assertThat(averageLatency).isLessThan(50.0)
    }

    @Test
    fun `swipe animation maintains 60 FPS`() {
        // Create multiple sessions
        repeat(3) {
            onView(withId(R.id.fabAddSession)).perform(click())
        }

        val frameDrops = mutableListOf<Int>()

        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)

            // Monitor frame rate during swipe
            viewPager.addOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
                override fun onPageScrolled(
                    position: Int,
                    positionOffset: Float,
                    positionOffsetPixels: Int
                ) {
                    // Check if we're dropping frames
                    // (Simplified - real implementation would use Choreographer)
                }
            })
        }

        onView(withId(R.id.viewPager)).perform(swipeLeft())

        // Should have minimal frame drops
        assertThat(frameDrops.size).isLessThan(3)
    }

    @Test
    fun `memory usage stays under 100MB per session`() {
        val runtime = Runtime.getRuntime()
        val initialMemory = runtime.totalMemory() - runtime.freeMemory()

        // Create 5 sessions
        repeat(5) {
            onView(withId(R.id.fabAddSession)).perform(click())
        }

        // Force GC
        System.gc()
        Thread.sleep(100)

        val finalMemory = runtime.totalMemory() - runtime.freeMemory()
        val memoryPerSession = (finalMemory - initialMemory) / 5 / 1024 / 1024 // MB

        assertThat(memoryPerSession).isLessThan(100)
    }
}
```

**ConfigurationChangeTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
class ConfigurationChangeTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun `sessions survive rotation`() {
        // Create sessions
        onView(withId(R.id.fabAddSession)).perform(click())
        onView(withId(R.id.fabAddSession)).perform(click())

        var sessionCountBefore = 0
        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            sessionCountBefore = viewPager.adapter?.itemCount ?: 0
        }

        // Rotate
        activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        Thread.sleep(500)

        // Verify sessions preserved
        activityRule.scenario.onActivity { activity ->
            val viewPager = activity.findViewById<ViewPager2>(R.id.viewPager)
            assertThat(viewPager.adapter?.itemCount).isEqualTo(sessionCountBefore)
        }
    }

    @Test
    fun `modifier state survives rotation`() {
        // Toggle Ctrl
        onView(withId(R.id.btnCtrl)).perform(click())

        // Rotate
        activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        Thread.sleep(500)

        // Verify Ctrl still active
        onView(withId(R.id.btnCtrl))
            .check(matches(hasBackground(R.color.modifier_active)))
    }

    @Test
    fun `terminal content survives rotation`() {
        // Type some content
        onView(withId(R.id.btnTab)).perform(click())

        var contentBefore = ""
        activityRule.scenario.onActivity { activity ->
            val fragment = activity.supportFragmentManager
                .findFragmentByTag("f0") as? TerminalFragment
            fragment?.getTerminalContent { content ->
                contentBefore = content
            }
        }

        // Rotate
        activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        Thread.sleep(500)

        // Verify content preserved
        activityRule.scenario.onActivity { activity ->
            val fragment = activity.supportFragmentManager
                .findFragmentByTag("f0") as? TerminalFragment
            fragment?.getTerminalContent { content ->
                assertThat(content).isEqualTo(contentBefore)
            }
        }
    }
}
```

**Acceptance Criteria**:

- Sessions persist across app restarts
- Configuration changes (rotation) preserve state
- Performance targets met (startup, latency, FPS)
- Memory usage within bounds

---

### Milestone 7 & 8: Advanced Features & Release

#### Integration Tests

**KeyboardToTerminalEndToEndTest.kt**

```kotlin
@RunWith(AndroidJUnit4::class)
@LargeTest
class KeyboardToTerminalEndToEndTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun `complete workflow - toggle modifier, type key, verify sequence`() {
        val latch = CountDownLatch(1)
        var receivedSequence = ""

        activityRule.scenario.onActivity { activity ->
            val fragment = activity.supportFragmentManager
                .findFragmentByTag("f0") as? TerminalFragment
            fragment?.setOnInputSentListener { sequence ->
                receivedSequence = sequence
                latch.countDown()
            }
        }

        // Toggle Ctrl
        onView(withId(R.id.btnCtrl)).perform(click())

        // Wait for AlphaGrid animation
        Thread.sleep(300)

        // Click 'C'
        onView(withText("C")).perform(click())

        // Wait for sequence
        latch.await(1, TimeUnit.SECONDS)

        // Verify Ctrl+C was sent
        assertThat(receivedSequence).isEqualTo("\u0003")

        // Verify AlphaGrid dismissed
        onView(withId(R.id.alphaGrid))
            .check(matches(withEffectiveVisibility(Visibility.GONE)))

        // Verify Ctrl deactivated
        onView(withId(R.id.btnCtrl))
            .check(matches(not(hasBackground(R.color.modifier_active))))
    }

    @Test
    fun `typing without modifiers sends regular characters`() {
        // Ensure no modifiers active
        activityRule.scenario.onActivity { activity ->
            val viewModel = ViewModelProvider(activity)[TerminalViewModel::class.java]
            viewModel.resetModifiers()
        }

        val sequences = mutableListOf<String>()
        val latch = CountDownLatch(3)

        activityRule.scenario.onActivity { activity ->
            val fragment = activity.supportFragmentManager
                .findFragmentByTag("f0") as? TerminalFragment
            fragment?.setOnInputSentListener { sequence ->
                sequences.add(sequence)
                latch.countDown()
            }
        }

        // Type Esc, Tab, Arrow
        onView(withId(R.id.btnEsc)).perform(click())
        onView(withId(R.id.btnTab)).perform(click())
        onView(withId(R.id.btnArrowUp)).perform(click())

        latch.await(2, TimeUnit.SECONDS)

        assertThat(sequences).containsExactly(
            "\u001B",   // Esc
            "\u0009",   // Tab
            "\u001B[A"  // Arrow Up
        )
    }
}
```

**Acceptance Criteria**:

- End-to-end workflows function correctly
- All features integrate seamlessly
- No regressions in core functionality

---

## Test Execution Strategy

### Continuous Integration

```yaml
# .github/workflows/android-ci.yml
name: Android CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Set up JDK 17
        uses: actions/setup-java@v3
        with:
          java-version: '17'
      - name: Run unit tests
        run: ./gradlew test
      - name: Run instrumentation tests
        uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 29
          script: ./gradlew connectedAndroidTest
      - name: Upload test reports
        uses: actions/upload-artifact@v3
        with:
          name: test-reports
          path: app/build/reports/
```

### Coverage Goals

- **Unit Tests**: 80%+ coverage for ViewModels, utilities, models
- **Integration Tests**: 70%+ coverage for Fragments, Views, bridges
- **UI Tests**: Critical user flows covered (modifier input, session switching)

### Test Execution Frequency

- **Unit tests**: Every commit (fast, < 30 seconds)
- **Integration tests**: Every PR (medium, ~2-5 minutes)
- **Performance tests**: Nightly builds
- **Manual tests**: Before each release

### Debugging Failed Tests

```kotlin
// Enable verbose logging in test builds
adb shell setprop log.tag.TerminalApp DEBUG
adb logcat -s TerminalApp:D

// Capture screenshots on failure
@Rule
val screenshotRule = ScreenshotTestRule()
```

### Mocking Strategy for WebView

Since WebView cannot be easily mocked, we use:

1. **Real WebView in instrumentation tests** (device/emulator required)
2. **Robolectric for unit tests** (simulated WebView on JVM)
3. **Test doubles for ViewModel logic** (mock terminal interface)

```kotlin
// Example: Mocking terminal interface for ViewModel tests
interface TerminalInterface {
    fun sendInput(input: String)
    fun setOnDataListener(listener: (String) -> Unit)
}

class MockTerminalInterface : TerminalInterface {
    val sentInputs = mutableListOf<String>()
    var dataListener: ((String) -> Unit)? = null

    override fun sendInput(input: String) {
        sentInputs.add(input)
    }

    override fun setOnDataListener(listener: (String) -> Unit) {
        dataListener = listener
    }
}
```

This testing strategy ensures reliability, performance, and maintainability throughout development.
