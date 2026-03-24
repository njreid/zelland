package com.njr.zelland

import android.app.Activity
import android.view.HapticFeedbackConstants
import android.view.LayoutInflater
import android.view.View
import android.webkit.WebView
import android.widget.Button
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout

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
    private var arrowSubbar: View? = null

    internal var modCtrl = false
    internal var modAlt  = false
    internal var modMeta = false
    private var arrowsOpen = false

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

        val inflater = LayoutInflater.from(activity)
        val bar = inflater.inflate(R.layout.native_keybar, contentFrame, false)
        bar.tag = "keybar_root"
        keybarView = bar
        arrowSubbar = bar.findViewById(R.id.kb_arrow_row)
        arrowSubbar?.visibility = View.GONE

        // Wrap WRY's root view + keybar in a vertical LinearLayout so they
        // each occupy their own region with no z-order or elevation tricks.
        // WebView hardware surfaces bypass normal elevation/z-order compositing,
        // so the overlay approach (Gravity.TOP + topMargin) would hide the keybar
        // behind the WebView surface. A LinearLayout solves this cleanly.
        val wryRoot = contentFrame.getChildAt(0)
        if (wryRoot != null) {
            contentFrame.removeView(wryRoot)
            val wrapper = LinearLayout(activity).apply {
                orientation = LinearLayout.VERTICAL
            }
            wrapper.addView(bar, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ))
            wrapper.addView(wryRoot, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f
            ))
            contentFrame.addView(wrapper, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
        } else {
            // WRY root not found — fall back to overlay at top
            contentFrame.addView(bar, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                android.view.Gravity.TOP
            ))
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

        bar.findViewById<ImageButton>(R.id.kb_arrows_toggle).setOnClickListener {
            arrowsOpen = !arrowsOpen
            arrowSubbar?.visibility = if (arrowsOpen) View.VISIBLE else View.GONE
            updateArrowsToggleUI(bar)
        }

        bar.findViewById<ImageButton>(R.id.kb_menu).setOnClickListener {
            emit("kb-sidebar-toggle", "{}")
            haptic()
        }

        bar.findViewById<Button>(R.id.kb_esc).setOnClickListener         { sendSeq("\u001b") }
        bar.findViewById<ImageButton>(R.id.kb_enter).setOnClickListener  { sendSeq("\r") }
        bar.findViewById<ImageButton>(R.id.kb_tab).setOnClickListener    { sendSeq("\t") }

        bar.findViewById<Button>(R.id.kb_tab1).setOnClickListener { sendTabSwitch(1) }
        bar.findViewById<Button>(R.id.kb_tab2).setOnClickListener { sendTabSwitch(2) }
        bar.findViewById<Button>(R.id.kb_tab3).setOnClickListener { sendTabSwitch(3) }

        bar.findViewById<ImageButton>(R.id.kb_left).setOnClickListener   { sendArrow("D") }
        bar.findViewById<ImageButton>(R.id.kb_up).setOnClickListener     { sendArrow("A") }
        bar.findViewById<ImageButton>(R.id.kb_down).setOnClickListener   { sendArrow("B") }
        bar.findViewById<ImageButton>(R.id.kb_right).setOnClickListener  { sendArrow("C") }
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

    private fun resetModifiers() {
        modCtrl = false; modAlt = false; modMeta = false
        activity.runOnUiThread { keybarView?.let { updateModifierUI(it) } }
    }

    private fun haptic() {
        keybarView?.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    private fun updateModifierUI(bar: View) {
        val activeColor   = activity.getColor(R.color.kb_primary)
        val inactiveColor = activity.getColor(R.color.kb_button_bg)
        fun tint(id: Int, active: Boolean) =
            bar.findViewById<Button>(id).setBackgroundColor(if (active) activeColor else inactiveColor)
        tint(R.id.kb_ctrl, modCtrl)
        tint(R.id.kb_alt,  modAlt)
        tint(R.id.kb_meta, modMeta)
    }

    private fun updateArrowsToggleUI(bar: View) {
        val btn = bar.findViewById<ImageButton>(R.id.kb_arrows_toggle)
        btn.setBackgroundColor(
            if (arrowsOpen) activity.getColor(R.color.kb_primary)
            else            activity.getColor(R.color.kb_button_bg)
        )
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
