package com.njr.zelland

import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Rect
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.util.Base64
import android.util.Log
import android.view.ActionMode
import android.view.GestureDetector
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewGroup
import android.view.ViewTreeObserver
import android.content.ClipboardManager
import android.content.ClipData
import android.view.ScaleGestureDetector
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.webkit.WebView
import android.widget.EditText
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import androidx.core.view.GestureDetectorCompat
import androidx.core.view.ViewCompat
import javax.crypto.Cipher

class MainActivity : TauriActivity() {
    private lateinit var keyStoreManager: KeyStoreManager
    private lateinit var biometricManager: BiometricManager
    private var surfaceView: SurfaceView? = null
    private var webViewRef: WebView? = null
    internal var hiddenEditText: EditText? = null
    internal var keybarPlugin: KeybarPlugin? = null
    private lateinit var mDetector: GestureDetectorCompat

    // Selection state
    private var selectionActive = false
    private var selStartCol = 0; private var selStartRow = 0
    private var selEndCol = 0;   private var selEndRow = 0
    private var actionMode: ActionMode? = null

    // Pinch-to-zoom
    private lateinit var scaleDetector: ScaleGestureDetector
    private var baseFontSize = 38f  // physical pixels, matches CELL_HEIGHT default
    private var isPinching = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        keyStoreManager = KeyStoreManager()
        biometricManager = BiometricManager(this)
        copyFontsFromAssets()
        startForegroundService(Intent(this, TerminalSessionService::class.java))

        // Request POST_NOTIFICATIONS permission on Android 13+.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1001)
        }
        
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                webViewRef?.evaluateJavascript(
                    "window.dispatchEvent(new CustomEvent('kb-sidebar-toggle',{detail:{}}))",
                    null
                )
            }
        })

        mDetector = GestureDetectorCompat(this, object : GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                passTouchToRust("click", e.x, e.y)
                // Focus the hidden native EditText so the system keyboard attaches to it.
                val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
                hiddenEditText?.let { et ->
                    et.requestFocus()
                    imm.showSoftInput(et, 0)
                }
                return true
            }

            override fun onLongPress(e: MotionEvent) {
                val (col, row) = pixelToCell(e.x, e.y)
                selStartCol = col; selStartRow = row
                // Default selection: rest of the line from long-press point
                selEndCol = (col + 12).coerceAtMost(255); selEndRow = row
                selectionActive = true
                setSelectionHighlight(selStartCol, selStartRow, selEndCol, selEndRow, true)
                startSelectionActionMode()
            }

            override fun onFling(
                e1: MotionEvent?,
                e2: MotionEvent,
                velocityX: Float,
                velocityY: Float
            ): Boolean {
                // Left swipe (negative X velocity) → open sidebar
                if (velocityX < -600f && Math.abs(velocityX) > Math.abs(velocityY) * 1.5f) {
                    webViewRef?.evaluateJavascript(
                        "window.dispatchEvent(new CustomEvent('kb-sidebar-toggle',{detail:{}}))", null
                    )
                    return true
                }
                return false
            }

            override fun onScroll(
                e1: MotionEvent?,
                e2: MotionEvent,
                distanceX: Float,
                distanceY: Float
            ): Boolean {
                // Two-finger scroll only: distanceY positive = finger moved up = scroll down.
                if (e2.pointerCount == 2) {
                    val action = if (distanceY > 0) "scroll_down" else "scroll_up"
                    passTouchToRust(action, e2.x, e2.y)
                    return true
                }
                return false
            }
        })

        scaleDetector = ScaleGestureDetector(this, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                isPinching = true
                return true
            }
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                baseFontSize = (baseFontSize * detector.scaleFactor).coerceIn(20f, 80f)
                updateFontSizeToRust(baseFontSize)
                return true
            }
            override fun onScaleEnd(detector: ScaleGestureDetector) {
                isPinching = false
            }
        })
    }

    override fun onDestroy() {
        super.onDestroy()
        stopService(Intent(this, TerminalSessionService::class.java))
    }

    /**
     * Called when the app is already running and a notification action brings it to the
     * foreground. Reads the `navigate_session` extra and dispatches a JS CustomEvent so
     * the Svelte store can navigate to the right session.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val session = intent.getStringExtra("navigate_session") ?: return
        val safe = session.replace("\\", "\\\\").replace("'", "\\'")
        webViewRef?.post {
            webViewRef?.evaluateJavascript(
                "window.__navigateToSession && window.__navigateToSession('$safe')",
                null
            )
        }
    }

    override fun onWebViewCreate(webView: WebView) {
        webViewRef = webView
        // If the activity was started cold from a notification action, handle it now.
        intent?.getStringExtra("navigate_session")?.let { session ->
            val safe = session.replace("\\", "\\\\").replace("'", "\\'")
            webView.post {
                webView.evaluateJavascript(
                    "window.__navigateToSession && window.__navigateToSession('$safe')",
                    null
                )
            }
        }
        // JS bridge to show/hide the SurfaceView when the user swipes between panes.
        webView.addJavascriptInterface(object : Any() {
            @android.webkit.JavascriptInterface
            fun setVisible(visible: Boolean) {
                runOnUiThread {
                    surfaceView?.visibility = if (visible) android.view.View.VISIBLE else android.view.View.GONE
                }
            }
        }, "TerminalNative")
        // Make the WebView transparent so the wgpu SurfaceView terminal shows through.
        webView.setBackgroundColor(Color.TRANSPARENT)
        webView.isFocusableInTouchMode = true
        window.decorView.post {
            keybarPlugin = KeybarPlugin(this, webView).also { it.setup() }
            setupNativeSurface(webView)
        }
    }

    private fun setupNativeSurface(webView: WebView) {
        val parent = webView.parent as? ViewGroup ?: return
        val index = parent.indexOfChild(webView)
        
        val container = FrameLayout(this)
        container.layoutParams = webView.layoutParams

        // Add the container to the parent BEFORE populating it, so the layout system
        // resolves the container to its true screen dimensions. If we add the SurfaceView
        // first, surfaceChanged fires with the WebView's pre-inset size (2048px) instead
        // of the real height (~2349px), corrupting the initial wgpu surface configuration.
        parent.removeView(webView)
        parent.addView(container, index)

        surfaceView = SurfaceView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Start hidden — JS controls visibility via TerminalNative.setVisible().
            // Without this the SurfaceView covers the entire WebView immediately on
            // startup (before the Svelte $effect fires), blocking the welcome screen.
            visibility = android.view.View.GONE
            // Lift the SurfaceView above the Window layer so it composites on top
            // of the WebView rather than relying on the punch-through mechanism,
            // which silently fails in hardware-accelerated Tauri view hierarchies.
            setZOrderMediaOverlay(true)
            setOnTouchListener { _, event ->
                scaleDetector.onTouchEvent(event)

                if (selectionActive) {
                    when (event.actionMasked) {
                        MotionEvent.ACTION_MOVE -> {
                            val (col, row) = pixelToCell(event.x, event.y)
                            if (col != selEndCol || row != selEndRow) {
                                selEndCol = col; selEndRow = row
                                setSelectionHighlight(selStartCol, selStartRow, selEndCol, selEndRow, true)
                            }
                            return@setOnTouchListener true
                        }
                        MotionEvent.ACTION_UP -> return@setOnTouchListener true
                    }
                }

                if (!isPinching) {
                    mDetector.onTouchEvent(event)
                }
                true
            }
            holder.addCallback(object : android.view.SurfaceHolder.Callback {
                override fun surfaceCreated(holder: android.view.SurfaceHolder) {
                    Log.d("MainActivity", "surfaceCreated")
                    webViewRef?.post {
                        webViewRef?.evaluateJavascript(
                            "window.dispatchEvent(new CustomEvent('surface-ready',{detail:{}}))", null
                        )
                    }
                }
                override fun surfaceChanged(holder: android.view.SurfaceHolder, format: Int, width: Int, height: Int) {}
                override fun surfaceDestroyed(holder: android.view.SurfaceHolder) {
                    Log.d("MainActivity", "surfaceDestroyed")
                    webViewRef?.post {
                        webViewRef?.evaluateJavascript(
                            "window.dispatchEvent(new CustomEvent('surface-unavailable',{detail:{}}))", null
                        )
                    }
                }
            })
        }
        
        val fillParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        // WebView must be added FIRST so the SurfaceView (added second) is
        // last in the FrameLayout child list. Android dispatches touch events
        // to the last-added child first, so the SurfaceView intercepts touches
        // when VISIBLE. When GONE (no active session) it is excluded from
        // dispatch automatically and all touches reach the WebView as normal.
        // NOTE: setZOrderMediaOverlay(true) controls rendering z-order
        // independently of the view-hierarchy order used for touch dispatch.
        container.addView(webView, FrameLayout.LayoutParams(fillParams))
        container.addView(surfaceView, fillParams)

        // Hidden 1×1 EditText positioned off-screen — provides a real InputConnection
        // so the system keyboard reliably shows and delivers text to the terminal.
        val imeIgnore = booleanArrayOf(false)
        hiddenEditText = EditText(this).apply {
            layoutParams = FrameLayout.LayoutParams(1, 1)
            translationX = -9999f
            setBackgroundColor(Color.TRANSPARENT)
            setTextColor(Color.TRANSPARENT)
            isCursorVisible = false
            inputType = InputType.TYPE_CLASS_TEXT or
                        InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
                        InputType.TYPE_TEXT_FLAG_MULTI_LINE
            // IME_ACTION_NONE: Enter key inserts newline instead of dismissing the keyboard.
            imeOptions = EditorInfo.IME_ACTION_NONE or
                         EditorInfo.IME_FLAG_NO_FULLSCREEN or
                         EditorInfo.IME_FLAG_NO_EXTRACT_UI
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence, start: Int, count: Int, after: Int) {}
                override fun onTextChanged(s: CharSequence, start: Int, before: Int, count: Int) {}
                override fun afterTextChanged(s: Editable) {
                    if (imeIgnore[0]) return
                    val text = s.toString()
                    if (text.isEmpty()) return
                    val plugin = keybarPlugin
                    val ctrl = plugin?.modCtrl == true
                    val alt  = plugin?.modAlt  == true
                    for (ch in text) {
                        val seq = when {
                            ctrl && ch.isLetter() ->
                                (ch.lowercaseChar().code and 0x1f).toChar().toString()
                            ctrl && ch == '[' -> "\u001b"   // Ctrl+[ = ESC
                            ctrl && ch == '\\' -> "\u001c"
                            ctrl && ch == ']' -> "\u001d"
                            ctrl && ch == '^' -> "\u001e"
                            ctrl && ch == '_' -> "\u001f"
                            ctrl && ch == '@' -> "\u0000"   // Ctrl+@ = NUL
                            alt -> "\u001b${ch}"
                            ch == '\n' -> "\r"
                            else -> ch.toString()
                        }
                        val js = "window.dispatchEvent(new CustomEvent('kb-input',{detail:{seq:${seq.toJsStr()}}}));"
                        webViewRef?.evaluateJavascript(js, null)
                    }
                    plugin?.resetModifiers()
                    imeIgnore[0] = true
                    s.clear()
                    imeIgnore[0] = false
                }
            })
            setOnKeyListener { _, keyCode, event ->
                if (event.action == KeyEvent.ACTION_DOWN) {
                    val plugin = keybarPlugin
                    val ctrl = plugin?.modCtrl == true
                    val alt  = plugin?.modAlt  == true
                    val meta = plugin?.modMeta == true
                    
                    var seq = when (keyCode) {
                        KeyEvent.KEYCODE_DEL         -> "\u007f"
                        KeyEvent.KEYCODE_FORWARD_DEL -> "\u001b[3~"
                        KeyEvent.KEYCODE_ENTER       -> "\r"
                        KeyEvent.KEYCODE_TAB         -> "\t"
                        KeyEvent.KEYCODE_ESCAPE      -> "\u001b"
                        KeyEvent.KEYCODE_DPAD_UP     -> KeybarSeqs.modifiedArrow("A", ctrl, alt, meta)
                        KeyEvent.KEYCODE_DPAD_DOWN   -> KeybarSeqs.modifiedArrow("B", ctrl, alt, meta)
                        KeyEvent.KEYCODE_DPAD_RIGHT  -> KeybarSeqs.modifiedArrow("C", ctrl, alt, meta)
                        KeyEvent.KEYCODE_DPAD_LEFT   -> KeybarSeqs.modifiedArrow("D", ctrl, alt, meta)
                        else -> null
                    }
                    
                    // Handle modified Enter/Tab/Esc if needed, though usually just raw.
                    if (seq != null && (ctrl || alt || meta)) {
                        if (keyCode == KeyEvent.KEYCODE_ENTER) {
                            if (ctrl) seq = "\n" // Ctrl+Enter often LF
                            if (alt)  seq = "\u001b\r"
                        }
                    }

                    if (seq != null) {
                        val js = "window.dispatchEvent(new CustomEvent('kb-input',{detail:{seq:${seq.toJsStr()}}}));"
                        webViewRef?.evaluateJavascript(js, null)
                        plugin?.resetModifiers()
                        return@setOnKeyListener true
                    }
                }
                false
            }
        }
        container.addView(hiddenEditText)

        // Exclude the full SurfaceView from Android's system gesture navigation so that
        // left-swipe-from-right-edge is handled by our GestureDetector (opens sidebar)
        // rather than triggering the OS back gesture.
        // Keep this listener registered (no removeOnGlobalLayoutListener) so it also fires
        // on subsequent layout changes — e.g. keyboard show/hide — to keep the renderer sized
        // correctly. surfaceChanged reports a pre-inset size (2048px) on first fire, so we use
        // onGlobalLayout (which fires after insets settle) as the authoritative resize source.
        surfaceView?.viewTreeObserver?.addOnGlobalLayoutListener {
            val sv = surfaceView ?: return@addOnGlobalLayoutListener
            val w = sv.width; val h = sv.height
            if (w > 0 && h > 0) {
                ViewCompat.setSystemGestureExclusionRects(sv, listOf(Rect(0, 0, w, h)))
                passResizeToRust(w, h)
            }
        }

        surfaceView?.holder?.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                passSurfaceToRust(holder.surface)
            }
            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                // No-op: sizing is driven by onGlobalLayout above, not surfaceChanged,
                // because surfaceChanged fires with a stale pre-inset height on first call.
            }
            override fun surfaceDestroyed(holder: SurfaceHolder) {
                passSurfaceDestroyedToRust()
            }
        })
    }

    private fun copyFontsFromAssets() {
        val fontsDir = java.io.File(filesDir, "fonts")
        if (!fontsDir.exists()) fontsDir.mkdirs()
        val fonts = listOf(
            "NotoSansMNerdFontMono-Regular.ttf",
            "NotoSansMNerdFontMono-Bold.ttf"
        )
        for (name in fonts) {
            val dest = java.io.File(fontsDir, name)
            if (!dest.exists()) {
                try {
                    assets.open("fonts/$name").use { input ->
                        dest.outputStream().use { output -> input.copyTo(output) }
                    }
                    Log.d("MainActivity", "Copied font: $name")
                } catch (e: Exception) {
                    Log.e("MainActivity", "Failed to copy font $name: ${e.message}")
                }
            }
        }
    }

    private fun String.toJsStr(): String {
        val sb = StringBuilder("\"")
        for (c in this) {
            when (c) {
                '"'  -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> if (c.code < 0x20) sb.append("\\u%04x".format(c.code)) else sb.append(c)
            }
        }
        sb.append('"')
        return sb.toString()
    }

    private fun pixelToCell(x: Float, y: Float): Pair<Int, Int> {
        val dims = getCellDimensions()
        val cw = if (dims[0] > 0) dims[0] else 17f
        val ch = if (dims[1] > 0) dims[1] else 38f
        return Pair((x / cw).toInt().coerceAtLeast(0), (y / ch).toInt().coerceAtLeast(0))
    }

    private fun startSelectionActionMode() {
        actionMode?.finish()
        actionMode = (surfaceView ?: window.decorView).startActionMode(object : ActionMode.Callback {
            override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
                menu.add(0, 1, 0, android.R.string.copy)
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                menu.add(0, 2, 1, android.R.string.paste)
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                return true
            }
            override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false
            override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
                return when (item.itemId) {
                    1 -> { doCopy(); mode.finish(); true }
                    2 -> { doPaste(); mode.finish(); true }
                    else -> false
                }
            }
            override fun onDestroyActionMode(mode: ActionMode) {
                selectionActive = false
                setSelectionHighlight(0, 0, 0, 0, false)
                actionMode = null
            }
        }, ActionMode.TYPE_FLOATING)
    }

    private fun doCopy() {
        val text = getSelectionText(selStartCol, selStartRow, selEndCol, selEndRow)
        val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("terminal", text))
    }

    private fun doPaste() {
        val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
        val clip = cm.primaryClip ?: return
        if (clip.itemCount == 0) return
        val text = clip.getItemAt(0).coerceToText(this).toString()
        if (text.isEmpty()) return
        // Use bracketed paste mode so zellij/shell handles the paste safely
        val bracketed = "\u001b[200~$text\u001b[201~"
        passPasteToRust(bracketed.toByteArray(Charsets.UTF_8))
    }

    private external fun passSurfaceToRust(surface: Surface)
    private external fun passResizeToRust(width: Int, height: Int)
    private external fun passTouchToRust(action: String, x: Float, y: Float)
    private external fun passSurfaceDestroyedToRust()
    private external fun getSelectionText(sc: Int, sr: Int, ec: Int, er: Int): String
    private external fun setSelectionHighlight(sc: Int, sr: Int, ec: Int, er: Int, active: Boolean)
    private external fun passPasteToRust(data: ByteArray)
    private external fun getCellDimensions(): FloatArray
    private external fun updateFontSizeToRust(physicalPx: Float)

    fun generateBiometricKey(alias: String): Boolean {
        return try {
            keyStoreManager.generateBiometricKey(alias)
            true
        } catch (e: Exception) {
            false
        }
    }

    fun hasBiometricKey(alias: String): Boolean {
        return try {
            keyStoreManager.hasKey(alias)
        } catch (e: Exception) {
            false
        }
    }

    interface JniCallback {
        fun onComplete(success: Boolean, error: String?)
    }

    interface JniDecryptCallback {
        fun onComplete(success: Boolean, data: String?, error: String?)
    }

    fun authenticate(alias: String, title: String, subtitle: String, callback: JniCallback) {
        try {
            val cipher = keyStoreManager.getCipher(alias, Cipher.DECRYPT_MODE)
            biometricManager.authenticate(title, subtitle, cipher, object : BiometricManager.AuthCallback {
                override fun onSuccess(cipher: Cipher?) {
                    callback.onComplete(true, null)
                }
                override fun onError(err: String) {
                    callback.onComplete(false, err)
                }
            })
        } catch (e: Exception) {
            callback.onComplete(false, e.message)
        }
    }

    fun authenticateAndDecrypt(
        alias: String,
        title: String,
        subtitle: String,
        encryptedDataBase64: String,
        callback: JniDecryptCallback
    ) {
        try {
            val cipher = keyStoreManager.getCipher(alias, Cipher.DECRYPT_MODE)
            biometricManager.authenticate(title, subtitle, cipher, object : BiometricManager.AuthCallback {
                override fun onSuccess(authedCipher: Cipher?) {
                    try {
                        val encryptedData = Base64.decode(encryptedDataBase64, Base64.NO_WRAP)
                        val decrypted = keyStoreManager.decryptData(authedCipher!!, encryptedData)
                        val decryptedStr = String(decrypted, Charsets.UTF_8)
                        callback.onComplete(true, decryptedStr, null)
                    } catch (e: Exception) {
                        callback.onComplete(false, null, "Decryption failed: ${e.message}")
                    }
                }
                override fun onError(err: String) {
                    callback.onComplete(false, null, err)
                }
            })
        } catch (e: Exception) {
            callback.onComplete(false, null, e.message)
        }
    }

    fun encryptWithBiometricKey(alias: String, data: String): String? {
        return try {
            val (iv, encrypted) = keyStoreManager.encryptData(alias, data.toByteArray(Charsets.UTF_8))
            val combined = iv + encrypted
            Base64.encodeToString(combined, Base64.NO_WRAP)
        } catch (e: Exception) {
            null
        }
    }
}
