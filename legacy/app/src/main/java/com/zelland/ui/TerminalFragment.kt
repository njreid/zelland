package com.zelland.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Rect
import android.net.http.SslError
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.webkit.ConsoleMessage
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.ProgressBar
import android.widget.Toast
import androidx.core.content.ContextCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import com.zelland.R
import com.zelland.viewmodel.TerminalViewModel

/**
 * Fragment that displays a Zellij web terminal in a WebView
 */
class TerminalFragment : Fragment(), TerminalWebView.ModifierProvider {

    private val viewModel: TerminalViewModel by activityViewModels()
    private lateinit var webView: TerminalWebView
    private lateinit var progressBar: ProgressBar
    private var sessionId: String? = null

    // Quick keys
    private lateinit var btnEsc: Button
    private lateinit var btnTab: Button
    private lateinit var btnCtrl: Button
    private lateinit var btnAlt: Button
    private lateinit var btnMeta: Button
    private lateinit var btnDown: Button
    private lateinit var btnEnter: Button
    private lateinit var btnPgDn: Button
    
    // D-Pad buttons
    private lateinit var btnUp: Button
    private lateinit var btnLeft: Button
    private lateinit var btnRight: Button
    private lateinit var dpadOverlay: View

    // Modifier states
    private enum class ModifierState { OFF, ON, LOCKED }
    private var ctrlState = ModifierState.OFF
    private var altState = ModifierState.OFF
    private var metaState = ModifierState.OFF

    // Double tap timing
    private var lastCtrlTapTime = 0L
    private var lastAltTapTime = 0L
    private var lastMetaTapTime = 0L
    private val doubleTapThreshold = 300L

    // Long press detection for Down arrow
    private val handler = Handler(Looper.getMainLooper())
    private var isDpadVisible = false
    private val longPressRunnable = Runnable {
        showDpad()
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val view = inflater.inflate(R.layout.fragment_terminal, container, false)

        webView = view.findViewById(R.id.webView)
        progressBar = view.findViewById(R.id.progressBar)
        dpadOverlay = view.findViewById(R.id.dpadOverlay)

        // Initialize quick keys
        btnEsc = view.findViewById(R.id.btnEsc)
        btnTab = view.findViewById(R.id.btnTab)
        btnCtrl = view.findViewById(R.id.btnCtrl)
        btnAlt = view.findViewById(R.id.btnAlt)
        btnMeta = view.findViewById(R.id.btnMeta)
        btnDown = view.findViewById(R.id.btnDown)
        btnEnter = view.findViewById(R.id.btnEnter)
        btnPgDn = view.findViewById(R.id.btnPgDn)
        
        // Initialize D-Pad keys
        btnUp = view.findViewById(R.id.btnUp)
        btnLeft = view.findViewById(R.id.btnLeft)
        btnRight = view.findViewById(R.id.btnRight)

        webView.modifierProvider = this

        sessionId = arguments?.getString(ARG_SESSION_ID)

        setupWebView()
        setupQuickKeys()
        setupDpadLogic()
        loadSession()

        return view
    }

    override fun getMetaState(): Int {
        var state = 0
        if (ctrlState != ModifierState.OFF) {
            state = state or KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
        }
        if (altState != ModifierState.OFF) {
            state = state or KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON
        }
        if (metaState != ModifierState.OFF) {
            state = state or KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON
        }
        return state
    }

    override fun onModifierUsed() {
        // Reset one-shot modifiers
        var changed = false
        if (ctrlState == ModifierState.ON) {
            ctrlState = ModifierState.OFF
            changed = true
        }
        if (altState == ModifierState.ON) {
            altState = ModifierState.OFF
            changed = true
        }
        if (metaState == ModifierState.ON) {
            metaState = ModifierState.OFF
            changed = true
        }
        if (changed) {
            activity?.runOnUiThread { updateModifierVisuals() }
        }
    }

    private fun setupQuickKeys() {
        btnEsc.setOnClickListener { sendKey("Escape", KeyEvent.KEYCODE_ESCAPE) }
        btnTab.setOnClickListener { sendKey("Tab", KeyEvent.KEYCODE_TAB) }
        btnEnter.setOnClickListener { sendKey("Enter", KeyEvent.KEYCODE_ENTER) }
        btnPgDn.setOnClickListener { sendKey("PageDown", KeyEvent.KEYCODE_PAGE_DOWN) }
        
        btnPgDn.setOnLongClickListener {
            sendKey("PageUp", KeyEvent.KEYCODE_PAGE_UP)
            true
        }

        btnCtrl.setOnClickListener { 
            ctrlState = cycleModifierState(ctrlState, lastCtrlTapTime)
            lastCtrlTapTime = SystemClock.uptimeMillis()
            updateModifierVisuals()
            showKeyboard()
        }
        
        btnAlt.setOnClickListener { 
            altState = cycleModifierState(altState, lastAltTapTime)
            lastAltTapTime = SystemClock.uptimeMillis()
            updateModifierVisuals()
            showKeyboard()
        }

        btnMeta.setOnClickListener { 
            metaState = cycleModifierState(metaState, lastMetaTapTime)
            lastMetaTapTime = SystemClock.uptimeMillis()
            updateModifierVisuals()
            showKeyboard()
        }
    }

    private fun cycleModifierState(currentState: ModifierState, lastTapTime: Long): ModifierState {
        val now = SystemClock.uptimeMillis()
        return when (currentState) {
            ModifierState.OFF -> ModifierState.ON
            ModifierState.ON -> {
                if (now - lastTapTime < doubleTapThreshold) ModifierState.LOCKED else ModifierState.OFF
            }
            ModifierState.LOCKED -> ModifierState.OFF
        }
    }

    private fun showKeyboard() {
        webView.requestFocus()
        val imm = context?.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        imm?.showSoftInput(webView, InputMethodManager.SHOW_IMPLICIT)
    }

    private fun updateModifierVisuals() {
        updateButtonStyle(btnCtrl, ctrlState)
        updateButtonStyle(btnAlt, altState)
        updateButtonStyle(btnMeta, metaState)
    }

    private fun updateButtonStyle(button: Button, state: ModifierState) {
        val activeColor = ContextCompat.getColor(requireContext(), R.color.zelland_primary)
        val normalColor = ContextCompat.getColor(requireContext(), R.color.white)
        val lockedBg = activeColor
        val normalBg = ContextCompat.getColor(requireContext(), R.color.gray_dark)

        when (state) {
            ModifierState.OFF -> {
                button.setTextColor(normalColor)
                button.setBackgroundColor(normalBg)
            }
            ModifierState.ON -> {
                button.setTextColor(activeColor)
                button.setBackgroundColor(normalBg)
            }
            ModifierState.LOCKED -> {
                button.setTextColor(normalColor)
                button.setBackgroundColor(lockedBg)
            }
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setupDpadLogic() {
        btnDown.setOnTouchListener { v, event ->
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    handler.postDelayed(longPressRunnable, 400)
                    false
                }
                MotionEvent.ACTION_UP -> {
                    handler.removeCallbacks(longPressRunnable)
                    if (isDpadVisible) {
                        val x = event.rawX
                        val y = event.rawY
                        
                        when {
                            isPointInsideView(x, y, btnUp) -> sendKey("ArrowUp", KeyEvent.KEYCODE_DPAD_UP)
                            isPointInsideView(x, y, btnLeft) -> sendKey("ArrowLeft", KeyEvent.KEYCODE_DPAD_LEFT)
                            isPointInsideView(x, y, btnRight) -> sendKey("ArrowRight", KeyEvent.KEYCODE_DPAD_RIGHT)
                            isPointInsideView(x, y, btnDown) -> sendKey("ArrowDown", KeyEvent.KEYCODE_DPAD_DOWN)
                        }
                        hideDpad()
                        true
                    } else {
                        v.performClick()
                        sendKey("ArrowDown", KeyEvent.KEYCODE_DPAD_DOWN)
                        true
                    }
                }
                MotionEvent.ACTION_CANCEL -> {
                    handler.removeCallbacks(longPressRunnable)
                    hideDpad()
                    false
                }
                else -> false
            }
        }
    }

    private fun showDpad() {
        isDpadVisible = true
        dpadOverlay.visibility = View.VISIBLE
        btnDown.isPressed = true
    }

    private fun hideDpad() {
        isDpadVisible = false
        dpadOverlay.visibility = View.GONE
        btnDown.isPressed = false
    }

    private fun isPointInsideView(x: Float, y: Float, view: View): Boolean {
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        val rect = Rect(location[0], location[1], location[0] + view.width, location[1] + view.height)
        return rect.contains(x.toInt(), y.toInt())
    }

    private fun sendKey(key: String, keyCode: Int) {
        val metaState = getMetaState()

        if (keyCode != 0) {
            val downTime = SystemClock.uptimeMillis()
            webView.dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_DOWN, keyCode, 0, metaState))
            webView.dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_UP, keyCode, 0, metaState))
        } else {
            val js = """
                (function() {
                    let target = document.querySelector('.xterm-helper-textarea') || document.activeElement || document.body;
                    const options = { 
                        key: '$key', 
                        ctrlKey: ${(metaState and KeyEvent.META_CTRL_ON) != 0}, 
                        altKey: ${(metaState and KeyEvent.META_ALT_ON) != 0}, 
                        metaKey: ${(metaState and KeyEvent.META_META_ON) != 0},
                        bubbles: true 
                    };
                    target.dispatchEvent(new KeyboardEvent('keydown', options));
                    target.dispatchEvent(new KeyboardEvent('keyup', options));
                })();
            """.trimIndent()
            webView.evaluateJavascript(js, null)
        }
        
        // Reset one-shot modifiers after bar buttons are used directly too
        onModifierUsed()
    }

    @SuppressLint("SetJavaScriptEnabled", "ClickableViewAccessibility")
    private fun setupWebView() {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            allowFileAccess = true
            allowContentAccess = true
            setSupportZoom(true)
            builtInZoomControls = true
            displayZoomControls = false
            userAgentString = "Mozilla/5.0 (Linux; Android) AppleWebKit/537.36 Zelland/1.0"
            @Suppress("DEPRECATION")
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            loadWithOverviewMode = true
            useWideViewPort = true
        }

        // Ensure clicking the WebView opens the keyboard
        webView.setOnTouchListener { v, event ->
            if (event.action == MotionEvent.ACTION_UP) {
                showKeyboard()
            }
            false
        }

        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                progressBar.visibility = View.GONE
                
                val css = """
                    @font-face { font-family: 'Hack'; src: local('monospace'); }
                    body, .terminal, .xterm { font-family: monospace !important; }
                    .xterm-helper-textarea { 
                        autocorrect: off !important; 
                        autocapitalize: off !important; 
                        spellcheck: false !important; 
                    }
                """.trimIndent()
                
                val js = """
                    var style = document.createElement('style');
                    style.innerHTML = `$css`;
                    document.head.appendChild(style);
                    
                    function fixTextArea() {
                        var ta = document.querySelector('.xterm-helper-textarea');
                        if (ta) {
                            ta.setAttribute('autocorrect', 'off');
                            ta.setAttribute('autocapitalize', 'off');
                            ta.setAttribute('spellcheck', 'false');
                        }
                    }
                    setInterval(fixTextArea, 1000);
                    fixTextArea();

                    new MutationObserver(function(mutations) {
                        if (document.title) { console.log('Title changed: ' + document.title); }
                    }).observe(document.querySelector('title'), { subtree: true, characterData: true, childList: true });
                """.trimIndent()
                view?.evaluateJavascript(js, null)
            }

            @SuppressLint("WebViewClientOnReceivedSslError")
            override fun onReceivedSslError(view: WebView?, handler: SslErrorHandler?, error: SslError?) {
                handler?.proceed()
            }
        }

        webView.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                super.onProgressChanged(view, newProgress)
                progressBar.visibility = if (newProgress < 100) View.VISIBLE else View.GONE
                progressBar.progress = newProgress
            }

            override fun onConsoleMessage(consoleMessage: ConsoleMessage?): Boolean {
                if (consoleMessage != null) {
                    val msg = consoleMessage.message()
                    if (msg.startsWith("Title changed:")) {
                        val title = msg.substringAfter("Title changed:").trim()
                        activity?.runOnUiThread { Toast.makeText(context, title, Toast.LENGTH_SHORT).show() }
                    }
                }
                return true
            }
        }
    }

    private fun loadSession() {
        val id = sessionId ?: return
        viewModel.sessions.observe(viewLifecycleOwner) { sessions ->
            val session = sessions.find { it.id == id }
            if (session != null && session.isConnected && session.localUrl != null) {
                webView.loadUrl(session.localUrl)
            }
        }
    }

    override fun onPause() {
        super.onPause()
        webView.onPause()
    }

    override fun onResume() {
        super.onResume()
        webView.onResume()
    }

    override fun onDestroyView() {
        super.onDestroyView()
        webView.destroy()
    }

    companion object {
        private const val ARG_SESSION_ID = "session_id"
        fun newInstance(sessionId: String): TerminalFragment = TerminalFragment().apply {
            arguments = Bundle().apply { putString(ARG_SESSION_ID, sessionId) }
        }
    }
}
