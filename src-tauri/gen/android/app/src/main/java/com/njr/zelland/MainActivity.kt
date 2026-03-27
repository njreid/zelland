package com.njr.zelland

import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Rect
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.util.Base64
import android.util.Log
import android.view.ActionMode
import android.view.GestureDetector
import android.view.Gravity
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
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
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.activity.OnBackPressedCallback
import androidx.core.view.GestureDetectorCompat
import androidx.core.view.ViewCompat
import androidx.drawerlayout.widget.DrawerLayout
import org.json.JSONArray
import org.json.JSONObject
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

    // Native sidebar
    private var drawerLayout: DrawerLayout? = null
    private var sidebarSessionsList: LinearLayout? = null
    private var sidebarTrashMode = false
    private var sidebarTrashBtn: TextView? = null
    private val expandedHostIds = mutableSetOf<String>()
    private var lastSidebarJson: String? = null

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
                val dl = drawerLayout
                if (dl != null && dl.isDrawerOpen(Gravity.START)) {
                    dl.closeDrawer(Gravity.START)
                } else {
                    webViewRef?.evaluateJavascript(
                        "window.dispatchEvent(new CustomEvent('kb-sidebar-toggle',{detail:{}}))",
                        null
                    )
                }
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
                // Left swipe (negative X velocity) → open native DrawerLayout sidebar
                if (velocityX < -600f && Math.abs(velocityX) > Math.abs(velocityY) * 1.5f) {
                    drawerLayout?.openDrawer(Gravity.START)
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
                    surfaceView?.visibility = if (visible) View.VISIBLE else View.GONE
                }
            }
        }, "TerminalNative")
        // JS bridge for native sidebar data and actions.
        webView.addJavascriptInterface(object : Any() {
            @android.webkit.JavascriptInterface
            fun updateData(json: String) {
                updateNativeSidebarData(json)
            }
            @android.webkit.JavascriptInterface
            fun openDrawer() {
                runOnUiThread { drawerLayout?.openDrawer(Gravity.START) }
            }
            @android.webkit.JavascriptInterface
            fun closeDrawer() {
                runOnUiThread { drawerLayout?.closeDrawer(Gravity.START) }
            }
        }, "SidebarNative")
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

        // DrawerLayout wraps everything: main content + left sidebar panel.
        val dl = DrawerLayout(this)
        dl.layoutParams = webView.layoutParams

        // Main content container: WebView + SurfaceView + hidden EditText stacked.
        val container = FrameLayout(this)
        container.layoutParams = DrawerLayout.LayoutParams(
            DrawerLayout.LayoutParams.MATCH_PARENT,
            DrawerLayout.LayoutParams.MATCH_PARENT
        )

        // Insert the DrawerLayout into the parent at the WebView's old position.
        parent.removeView(webView)
        parent.addView(dl, index)
        dl.addView(container)

        // Build and attach the native sidebar panel.
        val sidebarPanel = createSidebarPanel()
        dl.addView(sidebarPanel)
        drawerLayout = dl

        // Drawer listener: hide keyboard + keybar when drawer opens.
        dl.addDrawerListener(object : DrawerLayout.DrawerListener {
            override fun onDrawerSlide(drawerView: View, slideOffset: Float) {}
            override fun onDrawerOpened(drawerView: View) {
                val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
                hiddenEditText?.let { imm.hideSoftInputFromWindow(it.windowToken, 0) }
                webViewRef?.post {
                    webViewRef?.evaluateJavascript(
                        "(window.KeybarNative?.setVisible(false), void 0)", null
                    )
                }
            }
            override fun onDrawerClosed(drawerView: View) {
                sidebarTrashMode = false
                sidebarTrashBtn?.setTextColor(Color.parseColor("#a9b1d6"))
                webViewRef?.post {
                    webViewRef?.evaluateJavascript(
                        "window.dispatchEvent(new CustomEvent('native-drawer-closed'))", null
                    )
                }
            }
            override fun onDrawerStateChanged(newState: Int) {}
        })

        surfaceView = SurfaceView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Start hidden — JS controls visibility via TerminalNative.setVisible().
            visibility = View.GONE
            // Lift the SurfaceView above the Window layer so it composites on top
            // of the WebView rather than relying on the punch-through mechanism.
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
        // WebView first → SurfaceView second (last-added child gets touches first).
        container.addView(webView, FrameLayout.LayoutParams(fillParams))
        container.addView(surfaceView, fillParams)

        // Hidden 1×1 EditText for keyboard input.
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
                        val seq = KeySeqs.charToSeq(ch, ctrl, alt)
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

                    if (seq != null && (ctrl || alt || meta)) {
                        if (keyCode == KeyEvent.KEYCODE_ENTER) {
                            if (ctrl) seq = "\n"
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
            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}
            override fun surfaceDestroyed(holder: SurfaceHolder) {
                passSurfaceDestroyedToRust()
            }
        })
    }

    // ---------------------------------------------------------------------------
    // Native sidebar panel
    // ---------------------------------------------------------------------------

    private fun createSidebarPanel(): LinearLayout {
        val dp = resources.displayMetrics.density

        val panel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#13141e"))
            val widthPx = (300 * dp).toInt()
            val params = DrawerLayout.LayoutParams(widthPx, DrawerLayout.LayoutParams.MATCH_PARENT)
            params.gravity = Gravity.START
            layoutParams = params
        }

        // Header
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.parseColor("#0d0e17"))
            val pad = (16 * dp).toInt()
            setPadding(pad, pad, pad, pad)
            minimumHeight = (56 * dp).toInt()
            gravity = Gravity.CENTER_VERTICAL
        }
        header.addView(TextView(this).apply {
            text = "zelland"
            setTextColor(Color.parseColor("#7aa2f7"))
            textSize = 20f
            setTypeface(null, Typeface.BOLD)
        })
        panel.addView(header, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ))

        addDivider(panel, dp)

        // Scrollable sessions/hosts list
        val scrollView = ScrollView(this)
        sidebarSessionsList = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val pad = (8 * dp).toInt()
            setPadding(pad, pad, pad, pad)
        }
        scrollView.addView(sidebarSessionsList)
        panel.addView(scrollView, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f
        ))

        addDivider(panel, dp)

        // Footer: 4 action buttons
        val footer = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.parseColor("#0d0e17"))
            val hPad = (4 * dp).toInt()
            val vPad = (8 * dp).toInt()
            setPadding(hPad, vPad, hPad, vPad)
            weightSum = 4f
        }

        fun footerBtn(label: String, eventName: String): TextView {
            return TextView(this).apply {
                text = label
                setTextColor(Color.parseColor("#a9b1d6"))
                textSize = 11f
                gravity = Gravity.CENTER
                val pad = (6 * dp).toInt()
                setPadding(pad, (10 * dp).toInt(), pad, (10 * dp).toInt())
                setOnClickListener {
                    drawerLayout?.closeDrawer(Gravity.START)
                    webViewRef?.post {
                        webViewRef?.evaluateJavascript(
                            "window.dispatchEvent(new CustomEvent('$eventName'))", null
                        )
                    }
                }
            }
        }

        val btnParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        val trashBtn = TextView(this).apply {
            text = "🗑\nTrash"
            setTextColor(Color.parseColor("#a9b1d6"))
            textSize = 11f
            gravity = Gravity.CENTER
            val pad = (6 * dp).toInt()
            setPadding(pad, (10 * dp).toInt(), pad, (10 * dp).toInt())
            setOnClickListener {
                sidebarTrashMode = !sidebarTrashMode
                setTextColor(if (sidebarTrashMode) Color.parseColor("#f7768e") else Color.parseColor("#a9b1d6"))
                lastSidebarJson?.let { updateNativeSidebarData(it) }
            }
        }
        sidebarTrashBtn = trashBtn
        footer.addView(trashBtn, btnParams)
        footer.addView(footerBtn("⚠\nLog",     "native-show-errors"), LinearLayout.LayoutParams(btnParams))
        footer.addView(footerBtn("+\nHost",    "native-add-host"),    LinearLayout.LayoutParams(btnParams))
        footer.addView(footerBtn("+\nSession", "native-add-session"), LinearLayout.LayoutParams(btnParams))
        footer.addView(footerBtn("⚙\nSettings","native-settings"),    LinearLayout.LayoutParams(btnParams))

        panel.addView(footer, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ))

        return panel
    }

    private fun addDivider(parent: LinearLayout, dp: Float) {
        parent.addView(View(this).apply {
            setBackgroundColor(Color.parseColor("#292e42"))
        }, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, (1 * dp).toInt()))
    }

    /** Called from JS via SidebarNative.updateData(json). May be called off the UI thread. */
    internal fun updateNativeSidebarData(json: String) {
        lastSidebarJson = json
        val dp = resources.displayMetrics.density
        try {
            val obj = JSONObject(json)
            val favorites     = obj.optJSONArray("favorites")     ?: JSONArray()
            val projectHosts  = obj.optJSONArray("projectHosts")  ?: JSONArray()
            val savedSessions = obj.optJSONArray("savedSessions") ?: JSONArray()
            val activeId      = obj.optString("activeSessionId", "")

            runOnUiThread {
                val list = sidebarSessionsList ?: return@runOnUiThread
                list.removeAllViews()

                if (favorites.length() > 0) {
                    addSectionLabel(list, "FAVORITES", dp)
                    for (i in 0 until favorites.length()) {
                        val fav = favorites.getJSONObject(i)
                        if (fav.getString("type") == "project") {
                            addFavoriteProjectRow(list, fav.getString("hostId"),
                                fav.getString("projectName"), fav.optString("hostLabel", ""), dp)
                        } else {
                            addFavoriteSessionRow(list, fav.getString("id"), fav.getString("label"),
                                fav.optString("status", "connected"), fav.getString("id") == activeId, dp)
                        }
                    }
                }

                if (projectHosts.length() > 0) {
                    addSectionLabel(list, "PROJECT HOSTS", dp)
                    for (i in 0 until projectHosts.length()) {
                        val host = projectHosts.getJSONObject(i)
                        addProjectHostRow(list, host.getString("id"), host.getString("label"),
                            host.optString("status", "disconnected"),
                            host.optJSONArray("projects") ?: JSONArray(), dp)
                    }
                }

                if (savedSessions.length() > 0) {
                    addSectionLabel(list, "SAVED SESSIONS", dp)
                    for (i in 0 until savedSessions.length()) {
                        val s = savedSessions.getJSONObject(i)
                        addSavedSessionRow(list, s.getString("id"), s.getString("label"),
                            s.optString("status", "disconnected"), dp)
                    }
                }
            }
        } catch (e: Exception) {
            Log.e("MainActivity", "updateNativeSidebarData error: ${e.message}")
        }
    }

    private fun addSectionLabel(parent: LinearLayout, text: String, dp: Float) {
        parent.addView(TextView(this).apply {
            this.text = text
            textSize = 10f
            setTextColor(Color.parseColor("#565f89"))
            letterSpacing = 0.1f
            setPadding((8 * dp).toInt(), (12 * dp).toInt(), (8 * dp).toInt(), (4 * dp).toInt())
        }, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT
        ))
    }

    private fun dispatchJsEvent(eventName: String, detailJson: String) {
        webViewRef?.post {
            webViewRef?.evaluateJavascript(
                "window.dispatchEvent(new CustomEvent('$eventName',{detail:$detailJson}))", null
            )
        }
    }

    private fun statusDotColor(status: String): Int = when (status) {
        "connected"  -> Color.parseColor("#9ece6a")
        "connecting" -> Color.parseColor("#e0af68")
        "error"      -> Color.parseColor("#f7768e")
        else         -> Color.parseColor("#565f89")
    }

    private fun addStatusDot(parent: LinearLayout, color: Int, dp: Float) {
        val dotSize = (8 * dp).toInt()
        parent.addView(View(this).apply {
            background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(color) }
        }, LinearLayout.LayoutParams(dotSize, dotSize).apply {
            marginEnd = (10 * dp).toInt()
            gravity = Gravity.CENTER_VERTICAL
        })
    }

    private fun addFavoriteProjectRow(
        parent: LinearLayout, hostId: String, projectName: String, hostLabel: String, dp: Float
    ) {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((12 * dp).toInt(), (8 * dp).toInt(), (12 * dp).toInt(), (8 * dp).toInt())
            isClickable = true; isFocusable = true
            setOnClickListener {
                val safeHost = hostId.replace("'", "\\'")
                val safeName = projectName.replace("'", "\\'")
                if (sidebarTrashMode) {
                    dispatchJsEvent("native-unpin-project",
                        """{"hostId":"$safeHost","projectName":"$safeName"}""")
                } else {
                    drawerLayout?.closeDrawer(Gravity.START)
                    dispatchJsEvent("native-open-project",
                        """{"hostId":"$safeHost","projectName":"$safeName"}""")
                }
            }
        }
        row.addView(TextView(this).apply {
            text = "📌"; textSize = 13f
        }, LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT).apply { marginEnd = (8 * dp).toInt() })
        val textCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(TextView(this@MainActivity).apply {
                text = projectName; textSize = 14f
                setTextColor(Color.parseColor("#c0caf5"))
            })
            if (hostLabel.isNotEmpty()) {
                addView(TextView(this@MainActivity).apply {
                    text = hostLabel; textSize = 11f
                    setTextColor(Color.parseColor("#565f89"))
                })
            }
        }
        row.addView(textCol, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        if (sidebarTrashMode) {
            row.addView(TextView(this).apply { text = "✕"; textSize = 14f; setTextColor(Color.parseColor("#f7768e")) })
        }
        parent.addView(row, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT))
    }

    private fun addFavoriteSessionRow(
        parent: LinearLayout, id: String, label: String, status: String, isActive: Boolean, dp: Float
    ) {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((12 * dp).toInt(), (8 * dp).toInt(), (12 * dp).toInt(), (8 * dp).toInt())
            if (isActive) setBackgroundColor(Color.parseColor("#1f2035"))
            isClickable = true; isFocusable = true
            setOnClickListener {
                val safeId = id.replace("'", "\\'")
                if (sidebarTrashMode) {
                    dispatchJsEvent("native-delete-session", """{"id":"$safeId"}""")
                } else {
                    drawerLayout?.closeDrawer(Gravity.START)
                    dispatchJsEvent("native-connect-session", """{"id":"$safeId"}""")
                }
            }
        }
        addStatusDot(row, statusDotColor(status), dp)
        row.addView(TextView(this).apply {
            text = label; textSize = 14f
            setTextColor(if (isActive) Color.parseColor("#7aa2f7") else Color.parseColor("#c0caf5"))
            if (isActive) setTypeface(null, Typeface.BOLD)
        }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        if (sidebarTrashMode) {
            row.addView(TextView(this).apply { text = "✕"; textSize = 14f; setTextColor(Color.parseColor("#f7768e")) })
        }
        parent.addView(row, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT))
    }

    private fun addProjectHostRow(
        parent: LinearLayout, hostId: String, label: String,
        status: String, projects: JSONArray, dp: Float
    ) {
        val isExpanded = expandedHostIds.contains(hostId)
        val container = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        val childContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = if (isExpanded) View.VISIBLE else View.GONE
        }
        for (i in 0 until projects.length()) {
            val proj = projects.getJSONObject(i)
            addProjectChildRow(childContainer, hostId, proj.getString("name"),
                proj.optBoolean("pinned", false), dp)
        }

        val chevronView = TextView(this).apply {
            text = if (isExpanded) "▼" else "▶"
            textSize = 10f; setTextColor(Color.parseColor("#565f89"))
        }
        val headerRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((12 * dp).toInt(), (10 * dp).toInt(), (12 * dp).toInt(), (10 * dp).toInt())
            isClickable = true; isFocusable = true
            setOnClickListener {
                if (sidebarTrashMode) {
                    val safeId = hostId.replace("'", "\\'")
                    AlertDialog.Builder(this@MainActivity)
                        .setTitle("Remove Host")
                        .setMessage("Remove \"$label\"? Pinned projects from this host will be unpinned.")
                        .setPositiveButton("Remove") { _, _ ->
                            drawerLayout?.closeDrawer(Gravity.START)
                            dispatchJsEvent("native-delete-host", """{"hostId":"$safeId"}""")
                        }
                        .setNegativeButton("Cancel", null)
                        .show()
                } else {
                    if (expandedHostIds.contains(hostId)) {
                        expandedHostIds.remove(hostId)
                        childContainer.visibility = View.GONE
                        chevronView.text = "▶"
                    } else {
                        expandedHostIds.add(hostId)
                        childContainer.visibility = View.VISIBLE
                        chevronView.text = "▼"
                    }
                }
            }
        }
        headerRow.addView(chevronView, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT).apply { marginEnd = (8 * dp).toInt() })
        addStatusDot(headerRow, statusDotColor(status), dp)
        headerRow.addView(TextView(this).apply {
            text = label; textSize = 14f; setTextColor(Color.parseColor("#a9b1d6"))
        }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        if (sidebarTrashMode) {
            headerRow.addView(TextView(this).apply { text = "✕"; textSize = 14f; setTextColor(Color.parseColor("#f7768e")) })
        }

        container.addView(headerRow, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        container.addView(childContainer, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        parent.addView(container, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
    }

    private fun addProjectChildRow(
        parent: LinearLayout, hostId: String, projectName: String, pinned: Boolean, dp: Float
    ) {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((28 * dp).toInt(), (6 * dp).toInt(), (12 * dp).toInt(), (6 * dp).toInt())
            isClickable = true; isFocusable = true
            setOnClickListener {
                val safeHost = hostId.replace("'", "\\'")
                val safeName = projectName.replace("'", "\\'")
                dispatchJsEvent("native-toggle-pin-project",
                    """{"hostId":"$safeHost","projectName":"$safeName"}""")
            }
        }
        row.addView(TextView(this).apply {
            text = if (pinned) "📌" else "📎"; textSize = 13f
        }, LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT).apply { marginEnd = (8 * dp).toInt() })
        row.addView(TextView(this).apply {
            text = projectName; textSize = 13f
            setTextColor(if (pinned) Color.parseColor("#7aa2f7") else Color.parseColor("#a9b1d6"))
        }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        parent.addView(row, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
    }

    private fun addSavedSessionRow(
        parent: LinearLayout, id: String, label: String, status: String, dp: Float
    ) {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((12 * dp).toInt(), (8 * dp).toInt(), (12 * dp).toInt(), (8 * dp).toInt())
            isClickable = true; isFocusable = true
            setOnClickListener {
                val safeId = id.replace("'", "\\'")
                if (sidebarTrashMode) {
                    dispatchJsEvent("native-delete-session", """{"id":"$safeId"}""")
                } else {
                    drawerLayout?.closeDrawer(Gravity.START)
                    dispatchJsEvent("native-connect-session", """{"id":"$safeId"}""")
                }
            }
        }
        addStatusDot(row, statusDotColor(status), dp)
        row.addView(TextView(this).apply {
            text = label; textSize = 14f; setTextColor(Color.parseColor("#a9b1d6"))
        }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        if (sidebarTrashMode) {
            row.addView(TextView(this).apply { text = "✕"; textSize = 14f; setTextColor(Color.parseColor("#f7768e")) })
        }
        parent.addView(row, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
    }

    // ---------------------------------------------------------------------------
    // Selection & copy/paste
    // ---------------------------------------------------------------------------

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
        val bracketed = "\u001b[200~$text\u001b[201~"
        passPasteToRust(bracketed.toByteArray(Charsets.UTF_8))
    }

    // ---------------------------------------------------------------------------
    // Utilities
    // ---------------------------------------------------------------------------

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

    // ---------------------------------------------------------------------------
    // JNI declarations
    // ---------------------------------------------------------------------------

    private external fun passSurfaceToRust(surface: Surface)
    private external fun passResizeToRust(width: Int, height: Int)
    private external fun passTouchToRust(action: String, x: Float, y: Float)
    private external fun passSurfaceDestroyedToRust()
    private external fun getSelectionText(sc: Int, sr: Int, ec: Int, er: Int): String
    private external fun setSelectionHighlight(sc: Int, sr: Int, ec: Int, er: Int, active: Boolean)
    private external fun passPasteToRust(data: ByteArray)
    private external fun getCellDimensions(): FloatArray
    private external fun updateFontSizeToRust(physicalPx: Float)

    // ---------------------------------------------------------------------------
    // Biometric / KeyStore
    // ---------------------------------------------------------------------------

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
