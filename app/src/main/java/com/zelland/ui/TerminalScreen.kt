package com.zelland.ui

import android.annotation.SuppressLint
import android.os.SystemClock
import android.view.KeyEvent
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.webkit.ConsoleMessage
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.zelland.R
import com.zelland.model.TerminalSession
import com.zelland.viewmodel.TerminalViewModel
import kotlin.math.abs

@Composable
fun TerminalScreen(
    session: TerminalSession,
    onModifierUsed: () -> Unit,
    viewModel: TerminalViewModel
) {
    val context = LocalContext.current
    var webViewRef by remember { mutableStateOf<TerminalWebView?>(null) }
    
    var ctrlState by remember { mutableStateOf(ModifierState.OFF) }
    var altState by remember { mutableStateOf(ModifierState.OFF) }
    var metaState by remember { mutableStateOf(ModifierState.OFF) }
    
    var lastCtrlTapTime by remember { mutableLongStateOf(0L) }
    var lastAltTapTime by remember { mutableLongStateOf(0L) }
    var lastMetaTapTime by remember { mutableLongStateOf(0L) }
    val doubleTapThreshold = 300L

    val modifierProvider = remember {
        object : TerminalWebView.ModifierProvider {
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
                if (ctrlState == ModifierState.ON) ctrlState = ModifierState.OFF
                if (altState == ModifierState.ON) altState = ModifierState.OFF
                if (metaState == ModifierState.ON) metaState = ModifierState.OFF
                onModifierUsed()
            }
        }
    }

    fun dispatchKeyEventWithModifiers(keyCode: Int, modifiers: Int) {
        webViewRef?.let { webView ->
            val downTime = SystemClock.uptimeMillis()
            webView.dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_DOWN, keyCode, 0, modifiers))
            webView.dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_UP, keyCode, 0, modifiers))
        }
    }

    fun sendKey(key: String, keyCode: Int) {
        val metaState = modifierProvider.getMetaState()
        if (keyCode != 0) {
            dispatchKeyEventWithModifiers(keyCode, metaState)
        } else {
            webViewRef?.let { webView ->
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
        }
        modifierProvider.onModifierUsed()
    }

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        bottomBar = {
            TerminalQuickBar(
                ctrlState = ctrlState,
                altState = altState,
                metaState = metaState,
                onCtrlClick = {
                    val now = SystemClock.uptimeMillis()
                    ctrlState = cycleModifierState(ctrlState, lastCtrlTapTime, now, doubleTapThreshold)
                    lastCtrlTapTime = now
                    showKeyboard(context, webViewRef)
                },
                onAltClick = {
                    val now = SystemClock.uptimeMillis()
                    altState = cycleModifierState(altState, lastAltTapTime, now, doubleTapThreshold)
                    lastAltTapTime = now
                    showKeyboard(context, webViewRef)
                },
                onMetaClick = {
                    val now = SystemClock.uptimeMillis()
                    metaState = cycleModifierState(metaState, lastMetaTapTime, now, doubleTapThreshold)
                    lastMetaTapTime = now
                    showKeyboard(context, webViewRef)
                },
                onKeyClick = { key, code -> sendKey(key, code) },
                onToggleDpad = { isVisible ->
                    if (isVisible) hideKeyboard(context, webViewRef)
                }
            )
        }
    ) { innerPadding ->
        Box(modifier = Modifier
            .fillMaxSize()
            .padding(innerPadding)
            .background(Color(0xFF1E1E1E))
        ) {
            AndroidView(
                factory = { ctx ->
                    TerminalWebView(ctx).apply {
                        layoutParams = ViewGroup.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.MATCH_PARENT
                        )
                        this.modifierProvider = modifierProvider
                        this.onZellijTabSwipeListener = { direction ->
                            val keyCode = if (direction == TerminalWebView.SwipeDirection.LEFT) {
                                KeyEvent.KEYCODE_DPAD_LEFT
                            } else {
                                KeyEvent.KEYCODE_DPAD_RIGHT
                            }
                            dispatchKeyEventWithModifiers(keyCode, KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON)
                        }
                        setupWebView(this, context)
                        webViewRef = this
                        session.localUrl?.let { loadUrl(it) }
                    }
                },
                modifier = Modifier.fillMaxSize()
            )
        }
    }
}

@Composable
fun TerminalQuickBar(
    ctrlState: ModifierState,
    altState: ModifierState,
    metaState: ModifierState,
    onCtrlClick: () -> Unit,
    onAltClick: () -> Unit,
    onMetaClick: () -> Unit,
    onKeyClick: (String, Int) -> Unit,
    onToggleDpad: (Boolean) -> Unit
) {
    var showDpadOverlay by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xFF212121))
            .navigationBarsPadding()
    ) {
        if (showDpadOverlay) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically
            ) {
                QuickKeyButton("PgUp", fontSize = 9) { onKeyClick("PageUp", KeyEvent.KEYCODE_PAGE_UP) }
                DPadButton(Icons.AutoMirrored.Filled.KeyboardArrowLeft) { onKeyClick("ArrowLeft", KeyEvent.KEYCODE_DPAD_LEFT) }
                DPadButton(Icons.Default.KeyboardArrowUp) { onKeyClick("ArrowUp", KeyEvent.KEYCODE_DPAD_UP) }
                DPadButton(Icons.Default.KeyboardArrowDown) { onKeyClick("ArrowDown", KeyEvent.KEYCODE_DPAD_DOWN) }
                DPadButton(Icons.AutoMirrored.Filled.KeyboardArrowRight) { onKeyClick("ArrowRight", KeyEvent.KEYCODE_DPAD_RIGHT) }
                QuickKeyButton("PgDn", fontSize = 9) { onKeyClick("PageDown", KeyEvent.KEYCODE_PAGE_DOWN) }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically
        ) {
            QuickKeyButton("ESC", fontSize = 9) { onKeyClick("Escape", KeyEvent.KEYCODE_ESCAPE) }
            
            IconButton(
                onClick = { onKeyClick("Tab", KeyEvent.KEYCODE_TAB) },
                modifier = Modifier.size(40.dp)
            ) {
                Icon(painter = painterResource(id = R.drawable.ic_tab_pipe), contentDescription = "Tab", tint = Color.White)
            }

            ModifierButton("C", ctrlState, onCtrlClick)
            ModifierButton("A", altState, onAltClick)
            ModifierButton("M", metaState, onMetaClick)

            FourWayArrowButton(
                onKeySend = { key, code -> onKeyClick(key, code) },
                onClick = { 
                    showDpadOverlay = !showDpadOverlay
                    onToggleDpad(showDpadOverlay)
                }
            )

            IconButton(
                onClick = { onKeyClick("Enter", KeyEvent.KEYCODE_ENTER) },
                modifier = Modifier.size(40.dp)
            ) {
                Icon(painter = painterResource(id = R.drawable.ic_enter), contentDescription = "Enter", tint = Color.White)
            }
        }
    }
}

@Composable
fun DPadButton(icon: ImageVector, onClick: () -> Unit) {
    IconButton(
        onClick = onClick,
        modifier = Modifier
            .padding(horizontal = 4.dp)
            .background(Color(0xFF424242), RoundedCornerShape(4.dp))
            .size(44.dp)
    ) {
        Icon(imageVector = icon, contentDescription = null, tint = Color.White)
    }
}

@Composable
fun QuickKeyButton(text: String, fontSize: Int = 14, onClick: () -> Unit) {
    TextButton(
        onClick = onClick,
        contentPadding = PaddingValues(0.dp),
        modifier = Modifier
            .minimumInteractiveComponentSize()
            .widthIn(min = 40.dp)
    ) {
        Text(text, color = Color.White, fontSize = fontSize.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
fun ModifierButton(label: String, state: ModifierState, onClick: () -> Unit) {
    val backgroundColor = when (state) {
        ModifierState.LOCKED -> MaterialTheme.colorScheme.primary
        else -> Color(0xFF424242)
    }
    val contentColor = when (state) {
        ModifierState.OFF -> Color.White
        ModifierState.ON -> MaterialTheme.colorScheme.primary
        ModifierState.LOCKED -> Color.White
    }

    Box(
        modifier = Modifier
            .size(40.dp)
            .background(backgroundColor, RoundedCornerShape(4.dp))
            .pointerInput(Unit) {
                detectTapGestures { onClick() }
            },
        contentAlignment = Alignment.Center
    ) {
        Text(label, color = contentColor, fontWeight = FontWeight.Bold, fontSize = 16.sp)
    }
}

@Composable
fun FourWayArrowButton(onKeySend: (String, Int) -> Unit, onClick: () -> Unit) {
    var offsetX by remember { mutableStateOf(0f) }
    var offsetY by remember { mutableStateOf(0f) }
    val dragThreshold = 80f
    
    var activeDirection by remember { mutableStateOf<Int?>(null) }

    Box(
        modifier = Modifier
            .size(44.dp)
            .background(Color(0xFF424242), RoundedCornerShape(4.dp))
            .pointerInput(Unit) {
                detectTapGestures(onTap = { onClick() })
            }
            .pointerInput(Unit) {
                detectDragGestures(
                    onDragStart = { 
                        offsetX = 0f
                        offsetY = 0f
                        activeDirection = null
                    },
                    onDrag = { change, dragAmount ->
                        change.consume()
                        offsetX += dragAmount.x
                        offsetY += dragAmount.y
                        
                        activeDirection = when {
                            abs(offsetY) > abs(offsetX) && offsetY > dragThreshold -> KeyEvent.KEYCODE_DPAD_DOWN
                            abs(offsetY) > abs(offsetX) && offsetY < -dragThreshold -> KeyEvent.KEYCODE_DPAD_UP
                            abs(offsetX) > abs(offsetY) && offsetX > dragThreshold -> KeyEvent.KEYCODE_DPAD_RIGHT
                            abs(offsetX) > abs(offsetY) && offsetX < -dragThreshold -> KeyEvent.KEYCODE_DPAD_LEFT
                            else -> null
                        }
                    },
                    onDragEnd = {
                        when (activeDirection) {
                            KeyEvent.KEYCODE_DPAD_DOWN -> onKeySend("ArrowDown", KeyEvent.KEYCODE_DPAD_DOWN)
                            KeyEvent.KEYCODE_DPAD_UP -> onKeySend("ArrowUp", KeyEvent.KEYCODE_DPAD_UP)
                            KeyEvent.KEYCODE_DPAD_RIGHT -> onKeySend("ArrowRight", KeyEvent.KEYCODE_DPAD_RIGHT)
                            KeyEvent.KEYCODE_DPAD_LEFT -> onKeySend("ArrowLeft", KeyEvent.KEYCODE_DPAD_LEFT)
                        }
                        offsetX = 0f
                        offsetY = 0f
                        activeDirection = null
                    }
                )
            },
        contentAlignment = Alignment.Center
    ) {
        val tint = if (activeDirection != null) MaterialTheme.colorScheme.primary else Color.White
        Icon(
            painter = painterResource(id = R.drawable.ic_four_way_arrow),
            contentDescription = "Arrows",
            tint = tint,
            modifier = Modifier.size(24.dp)
        )
    }
}

private fun cycleModifierState(currentState: ModifierState, lastTapTime: Long, now: Long, threshold: Long): ModifierState {
    return when (currentState) {
        ModifierState.OFF -> ModifierState.ON
        ModifierState.ON -> {
            if (now - lastTapTime < threshold) ModifierState.LOCKED else ModifierState.OFF
        }
        ModifierState.LOCKED -> ModifierState.OFF
    }
}

private fun showKeyboard(context: android.content.Context, webView: WebView?) {
    webView?.requestFocus()
    val imm = context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE) as? InputMethodManager
    imm?.showSoftInput(webView, InputMethodManager.SHOW_IMPLICIT)
}

private fun hideKeyboard(context: android.content.Context, webView: WebView?) {
    val imm = context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE) as? InputMethodManager
    imm?.hideSoftInputFromWindow(webView?.windowToken, 0)
}

@SuppressLint("SetJavaScriptEnabled")
private fun setupWebView(webView: WebView, context: android.content.Context) {
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

    webView.webViewClient = object : WebViewClient() {
        override fun onPageFinished(view: WebView?, url: String?) {
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
            """.trimIndent()
            view?.evaluateJavascript(js, null)
        }

        @SuppressLint("WebViewClientOnReceivedSslError")
        override fun onReceivedSslError(view: WebView?, handler: SslErrorHandler?, error: android.net.http.SslError?) {
            handler?.proceed()
        }
    }

    webView.webChromeClient = object : WebChromeClient() {
        override fun onConsoleMessage(consoleMessage: ConsoleMessage?): Boolean {
            return true
        }
    }
}
