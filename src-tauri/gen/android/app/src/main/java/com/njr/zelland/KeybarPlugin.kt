package com.njr.zelland

import android.app.Activity
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.LayoutInflater
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.Button
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Native keyboard bar injected at the top of the Activity content frame.
 * Instantiated directly from MainActivity.onWebViewCreate — does not use
 * Tauri's plugin discovery mechanism.
 *
 * Events are sent to JS via webView.evaluateJavascript / CustomEvent so that
 * the JS side can listen with window.addEventListener without any Tauri IPC.
 */
class KeybarPlugin(private val activity: Activity, private val webView: WebView) {

    private var keybarView: View? = null

    internal var modCtrl = false
    internal var modAlt  = false
    internal var modMeta = false

    // Lock state: double-tap locks a modifier so it persists across key sends.
    private var modCtrlLocked = false
    private var modAltLocked  = false
    private var modMetaLocked = false

    private var lastCtrlTap = 0L
    private var lastAltTap  = 0L
    private var lastMetaTap = 0L
    private val doubleTapMs = 350L

    // Test seam — replaced in unit tests
    var emit: (name: String, jsonPayload: String) -> Unit = { name, payload ->
        val js = "window.dispatchEvent(new CustomEvent(${name.toJsString()}, {detail:$payload}));"
        activity.runOnUiThread { webView.evaluateJavascript(js, null) }
    }

    /** Called from MainActivity after decorView.post — WebView is attached by now. */
    fun setup() {
        // Already on the main thread (posted from decorView.post in MainActivity).
        val contentFrame = activity.window.decorView
            .findViewById<FrameLayout>(android.R.id.content)

        Log.d(TAG, "setup: contentFrame=$contentFrame childCount=${contentFrame?.childCount}")

        val inflater = LayoutInflater.from(activity)
        val bar = inflater.inflate(R.layout.native_keybar, contentFrame, false)
        bar.tag = "keybar_root"
        keybarView = bar

        // Wrap WRY's root view + keybar in a vertical LinearLayout so they
        // each occupy their own region with no z-order or elevation tricks.
        // WebView hardware surfaces bypass normal elevation/z-order compositing,
        // so the overlay approach (Gravity.TOP + topMargin) would hide the keybar
        // behind the WebView surface. A LinearLayout solves this cleanly.
        val wryRoot = contentFrame.getChildAt(0)
        Log.d(TAG, "setup: wryRoot=$wryRoot (${wryRoot?.javaClass?.simpleName})")

        if (wryRoot != null) {
            contentFrame.removeView(wryRoot)
            val wrapper = LinearLayout(activity).apply {
                orientation = LinearLayout.VERTICAL
            }

            // Adjust wrapper padding so the bar stays above the system keyboard (IME).
            ViewCompat.setOnApplyWindowInsetsListener(wrapper) { v, insets ->
                val imeInsets = insets.getInsets(WindowInsetsCompat.Type.ime())
                val systemInsets = insets.getInsets(WindowInsetsCompat.Type.systemBars())
                // Use the larger of IME or navigation bar height to avoid overlap.
                v.setPadding(0, 0, 0, java.lang.Math.max(imeInsets.bottom, systemInsets.bottom))
                insets
            }

            wrapper.addView(wryRoot, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f
            ))
            wrapper.addView(bar, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ))
            contentFrame.addView(wrapper, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
            Log.d(TAG, "setup: wrapped successfully — bar+wryRoot in LinearLayout")
        } else {
            // WRY root not found — fall back to overlay at top
            contentFrame.addView(bar, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                android.view.Gravity.BOTTOM
            ))
            Log.w(TAG, "setup: wryRoot was null — used fallback overlay at bottom")
        }

        webView.addJavascriptInterface(KeybarBridge(), "KeybarNative")
        setupButtons(bar)
        Log.d(TAG, "setup: complete")
    }

    /** JS interface — callable as window.KeybarNative.setVisible(bool) from the WebView. */
    inner class KeybarBridge {
        @JavascriptInterface
        fun setVisible(visible: Boolean) {
            activity.runOnUiThread {
                keybarView?.visibility = if (visible) View.VISIBLE else View.GONE
            }
        }
    }

    private fun toggleKeyboard() {
        val imm = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
            as InputMethodManager
        val et = (activity as? MainActivity)?.hiddenEditText
        if (et != null) {
            et.requestFocus()
            // restartInput ensures the connection is fresh before showing the IME.
            imm.restartInput(et)
            imm.showSoftInput(et, InputMethodManager.SHOW_IMPLICIT)
        } else {
            imm.toggleSoftInput(InputMethodManager.SHOW_FORCED, 0)
        }
    }

    companion object {
        private const val TAG = "KeybarPlugin"
    }

    private fun setupButtons(bar: View) {
        bar.findViewById<Button>(R.id.kb_ctrl).setOnClickListener {
            handleModifierTap("ctrl"); updateModifierUI(bar)
        }
        bar.findViewById<Button>(R.id.kb_alt).setOnClickListener {
            handleModifierTap("alt"); updateModifierUI(bar)
        }
        bar.findViewById<Button>(R.id.kb_meta).setOnClickListener {
            handleModifierTap("meta"); updateModifierUI(bar)
        }

        bar.findViewById<ImageButton>(R.id.kb_menu).setOnClickListener {
            toggleKeyboard()
            haptic()
        }

        bar.findViewById<Button>(R.id.kb_esc).setOnClickListener          { sendSeq("\u001b") }
        bar.findViewById<ImageButton>(R.id.kb_enter).setOnClickListener   { sendSeq("\r") }
        bar.findViewById<ImageButton>(R.id.kb_tab).setOnClickListener     { sendSeq("\t") }

        bar.findViewById<Button>(R.id.kb_tab1).setOnClickListener { sendTabSwitch(1) }
        bar.findViewById<Button>(R.id.kb_tab2).setOnClickListener { sendTabSwitch(2) }
        bar.findViewById<Button>(R.id.kb_tab3).setOnClickListener { sendTabSwitch(3) }

        bar.findViewById<ImageButton>(R.id.kb_left).setOnClickListener  { sendArrow("D") }
        bar.findViewById<ImageButton>(R.id.kb_up).setOnClickListener    { sendArrow("A") }
        bar.findViewById<ImageButton>(R.id.kb_down).setOnClickListener  { sendArrow("B") }
        bar.findViewById<ImageButton>(R.id.kb_right).setOnClickListener { sendArrow("C") }
    }

    private fun sendSeq(seq: String) {
        emit("kb-input", """{"seq":${seq.toJsString()}}""")
        resetModifiers()
        haptic()
    }

    private fun sendArrow(letter: String) {
        sendSeq(KeybarSeqs.modifiedArrow(letter, modCtrl, modAlt, modMeta))
    }

    private fun sendTabSwitch(n: Int) {
        emit("kb-go-to-tab", """{"tab":$n}""")
        haptic()
    }

    /** Single tap: toggle active. Double tap: lock. Tap while locked: unlock. */
    private fun handleModifierTap(mod: String) {
        val now = System.currentTimeMillis()
        when (mod) {
            "ctrl" -> {
                if (modCtrlLocked) {
                    modCtrl = false; modCtrlLocked = false
                } else if (modCtrl && now - lastCtrlTap < doubleTapMs) {
                    modCtrlLocked = true
                } else {
                    modCtrl = !modCtrl
                }
                lastCtrlTap = now
            }
            "alt" -> {
                if (modAltLocked) {
                    modAlt = false; modAltLocked = false
                } else if (modAlt && now - lastAltTap < doubleTapMs) {
                    modAltLocked = true
                } else {
                    modAlt = !modAlt
                }
                lastAltTap = now
            }
            "meta" -> {
                if (modMetaLocked) {
                    modMeta = false; modMetaLocked = false
                } else if (modMeta && now - lastMetaTap < doubleTapMs) {
                    modMetaLocked = true
                } else {
                    modMeta = !modMeta
                }
                lastMetaTap = now
            }
        }
    }

    /** After sending a sequence, reset unlocked modifiers; locked ones persist. */
    internal fun resetModifiers() {
        if (!modCtrlLocked) modCtrl = false
        if (!modAltLocked)  modAlt  = false
        if (!modMetaLocked) modMeta = false
        activity.runOnUiThread { keybarView?.let { updateModifierUI(it) } }
    }

    private fun haptic() {
        keybarView?.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    private fun updateModifierUI(bar: View) {
        val primary = activity.getColor(R.color.kb_primary)
        // Locked: full primary colour. Active (single-tap): 50 % alpha. Inactive: transparent.
        fun tint(id: Int, active: Boolean, locked: Boolean) {
            val btn = bar.findViewById<Button>(id)
            val color = when {
                locked -> primary
                active -> android.graphics.Color.argb(
                    128,
                    android.graphics.Color.red(primary),
                    android.graphics.Color.green(primary),
                    android.graphics.Color.blue(primary)
                )
                else -> android.graphics.Color.TRANSPARENT
            }
            btn.setBackgroundColor(color)
        }
        tint(R.id.kb_ctrl, modCtrl, modCtrlLocked)
        tint(R.id.kb_alt,  modAlt,  modAltLocked)
        tint(R.id.kb_meta, modMeta, modMetaLocked)
    }

    /** Escapes a Kotlin string for safe embedding as a JS string literal. */
    private fun String.toJsString(): String {
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
}
