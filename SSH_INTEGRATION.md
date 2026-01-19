# Zelland - SSH Integration with Zellij Web

## Overview
This document outlines the SSH connection strategy for **Zelland** (Zellij + Android) to integrate with Zellij's web client functionality.

**Zelland** bridges the gap between mobile devices and remote terminal sessions by establishing SSH tunnels to Zellij web servers, enabling full terminal multiplexer functionality on Android.

## Zellij Web Architecture

Based on the [Zellij web client documentation](https://zellij.dev/documentation/web-client.html), here's how it works:

### Connection Flow
1. **Start Zellij Web Server**: `zellij web` starts an embedded web server (default: `localhost:8082`)
2. **Session URLs**: Access sessions via `http://127.0.0.1:8082/session-name`
   - Existing session: attach to it
   - Past session: resurrect from serialized metadata
   - New name: create new session
3. **Communication**: Bidirectional WebSocket connections
   - Terminal channel: STDIN/STDOUT
   - Control channel: window size, config, logs, session switching
4. **Security**: Token-based authentication (generated and revoked)

### Android Integration Strategy

```
┌─────────────────────────────────────────────────┐
│                  Android App                     │
│  ┌───────────────────────────────────────────┐  │
│  │          WebView (xterm.js)               │  │
│  │     Loading: http://localhost:9999/       │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │       Local Port Forward                   │  │
│  │    localhost:9999 → remote:8082           │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │          SSH Tunnel (SSHJ)                 │  │
│  │    Connection to remote host               │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
                       ↓↑
┌─────────────────────────────────────────────────┐
│              Remote SSH Host                     │
│  ┌───────────────────────────────────────────┐  │
│  │      Zellij Web Server (port 8082)        │  │
│  │         Started via: zellij web           │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## SSH Library Comparison

### Evaluated Libraries

| Library | Status | Pros | Cons | Recommendation |
|---------|--------|------|------|----------------|
| **SSHJ** | ✅ Active (May 2025) | Modern API, type-safe, Apache 2.0, port forwarding support | Larger dependency | **RECOMMENDED** |
| **JSch** | ⚠️ Unmaintained | Lightweight, direct SSH | Original abandoned; fork available but less active | Not recommended |
| **Apache MINA SSHD** | ✅ Active | Comprehensive, enterprise-grade | Heavy (full server + client), complex | Overkill for client-only |

### Recommendation: SSHJ

**SSHJ** is the best choice for Android SSH integration:

- **Active Maintenance**: [Latest release May 2025](https://github.com/hierynomus/sshj)
- **Clean API**: Modern, type-safe Java API designed for SSH
- **Port Forwarding**: Built-in support for local/remote port forwarding
- **Android Compatible**: Works on Android with proper configuration
- **License**: Apache 2.0

**Sources**:
- [Java SSH Libraries Comparison](https://medium.com/@ldclakmal/comparison-of-commons-vfs-sshj-and-jsch-libraries-for-sftp-support-cd5a0db2fbce)
- [SSHJ Library Guide](https://www.javaguides.net/2024/05/guide-to-sshj-library-in-java.html)
- [SSHJ GitHub Repository](https://github.com/hierynomus/sshj)

## Implementation Plan

### 1. Add SSHJ Dependency

```kotlin
// build.gradle.kts
dependencies {
    implementation("com.hierynomus:sshj:0.39.0") // Check for latest version
    implementation("org.bouncycastle:bcprov-jdk18on:1.78") // Required for SSHJ
    implementation("org.bouncycastle:bcpkix-jdk18on:1.78")
    implementation("org.slf4j:slf4j-android:1.7.36") // Logging for SSHJ
}
```

### 2. SSH Connection Manager

```kotlin
package com.zelland.ssh

import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.connection.channel.direct.Session
import net.schmizz.sshj.transport.verification.PromiscuousVerifier
import java.io.IOException
import java.util.concurrent.TimeUnit

data class SSHConfig(
    val host: String,
    val port: Int = 22,
    val username: String,
    val password: String? = null,
    val privateKeyPath: String? = null,
    val privateKeyPassphrase: String? = null
)

class SSHConnectionManager {
    private var sshClient: SSHClient? = null
    private var session: Session? = null
    private var portForward: PortForwardHandle? = null

    /**
     * Connect to SSH host and authenticate
     */
    @Throws(IOException::class)
    fun connect(config: SSHConfig) {
        sshClient = SSHClient().apply {
            // For development: accept all host keys
            // TODO: Implement proper host key verification for production
            addHostKeyVerifier(PromiscuousVerifier())

            // Load known providers
            loadKnownHosts()

            // Connect
            connect(config.host, config.port)

            // Authenticate
            when {
                config.password != null -> {
                    authPassword(config.username, config.password)
                }
                config.privateKeyPath != null -> {
                    val keyProvider = if (config.privateKeyPassphrase != null) {
                        loadKeys(config.privateKeyPath, config.privateKeyPassphrase)
                    } else {
                        loadKeys(config.privateKeyPath)
                    }
                    authPublickey(config.username, keyProvider)
                }
                else -> {
                    throw IllegalArgumentException("Must provide either password or private key")
                }
            }
        }
    }

    /**
     * Start Zellij web server on remote host
     * Returns the remote port number (default 8082)
     */
    @Throws(IOException::class)
    suspend fun startZellijWeb(): Int {
        val session = sshClient?.startSession()
            ?: throw IllegalStateException("Not connected")

        try {
            // Check if zellij web is already running
            val checkCmd = session.exec("pgrep -f 'zellij web' || echo 'not_running'")
            checkCmd.join(5, TimeUnit.SECONDS)
            val output = checkCmd.inputStream.bufferedReader().readText()

            if (output.trim() != "not_running") {
                // Already running
                return 8082 // Default Zellij web port
            }

            // Start zellij web in background
            // Note: Using nohup to keep it running after session closes
            val startCmd = session.exec("nohup zellij web > /tmp/zellij-web.log 2>&1 &")
            startCmd.join(2, TimeUnit.SECONDS)

            // Wait a moment for server to start
            kotlinx.coroutines.delay(1000)

            // Verify it started
            val verifyCmd = session.exec("pgrep -f 'zellij web'")
            verifyCmd.join(2, TimeUnit.SECONDS)
            val pid = verifyCmd.inputStream.bufferedReader().readText().trim()

            if (pid.isEmpty()) {
                throw IOException("Failed to start zellij web")
            }

            return 8082 // Zellij default port
        } finally {
            session.close()
        }
    }

    /**
     * Set up local port forwarding: localhost:localPort -> remoteHost:remotePort
     * This tunnels the Zellij web server to the Android device
     */
    @Throws(IOException::class)
    fun setupPortForward(localPort: Int, remotePort: Int): PortForwardHandle {
        val client = sshClient ?: throw IllegalStateException("Not connected")

        // Start local port forwarding
        val forwarder = client.newLocalPortForwarder(
            /* localAddr = */ null, // Listen on all interfaces
            /* localPort = */ localPort,
            /* remoteAddr = */ "localhost",
            /* remotePort = */ remotePort
        )

        // This needs to be run in a background thread
        val handle = PortForwardHandle(forwarder, localPort, remotePort)
        portForward = handle

        return handle
    }

    /**
     * Get the local URL for accessing Zellij web
     */
    fun getZellijUrl(sessionName: String = "default"): String {
        val localPort = portForward?.localPort
            ?: throw IllegalStateException("Port forwarding not set up")
        return "http://127.0.0.1:$localPort/$sessionName"
    }

    /**
     * Stop Zellij web server on remote host
     */
    @Throws(IOException::class)
    suspend fun stopZellijWeb() {
        val session = sshClient?.startSession()
            ?: throw IllegalStateException("Not connected")

        try {
            val stopCmd = session.exec("pkill -f 'zellij web'")
            stopCmd.join(2, TimeUnit.SECONDS)
        } finally {
            session.close()
        }
    }

    /**
     * Disconnect and clean up
     */
    fun disconnect() {
        try {
            portForward?.close()
            portForward = null

            session?.close()
            session = null

            sshClient?.disconnect()
            sshClient = null
        } catch (e: IOException) {
            // Log but don't throw
            e.printStackTrace()
        }
    }

    fun isConnected(): Boolean {
        return sshClient?.isConnected == true && sshClient?.isAuthenticated == true
    }
}

/**
 * Handle for port forwarding, must be kept alive while forwarding is active
 */
class PortForwardHandle(
    private val forwarder: net.schmizz.sshj.connection.channel.forwarded.LocalPortForwarder,
    val localPort: Int,
    val remotePort: Int
) : AutoCloseable {

    fun start() {
        // Port forwarding runs until close() is called
        // This should be called in a background thread
        forwarder.listen()
    }

    override fun close() {
        forwarder.close()
    }
}
```

### 3. Updated TerminalSession Model

```kotlin
data class TerminalSession(
    val id: String,
    val title: String,
    val sshConfig: SSHConfig? = null,
    val zellijSessionName: String = "session-${id.take(8)}",
    val isConnected: Boolean = false,
    val localUrl: String? = null // e.g., "http://127.0.0.1:9999/my-session"
)
```

### 4. Updated TerminalViewModel

```kotlin
class TerminalViewModel : ViewModel() {
    private val sshManagers = mutableMapOf<String, SSHConnectionManager>()

    val sessions = MutableLiveData<List<TerminalSession>>()
    val activeSessionIndex = MutableLiveData<Int>()
    val modifierState = MutableLiveData<ModifierState>()

    /**
     * Connect to SSH and start Zellij web for a session
     */
    suspend fun connectSession(sessionId: String, sshConfig: SSHConfig) {
        val manager = SSHConnectionManager()

        try {
            // Step 1: Connect via SSH
            withContext(Dispatchers.IO) {
                manager.connect(sshConfig)
            }

            // Step 2: Start Zellij web server
            val remotePort = withContext(Dispatchers.IO) {
                manager.startZellijWeb()
            }

            // Step 3: Set up port forwarding
            val localPort = findAvailablePort() // Find free port on device
            val portForward = withContext(Dispatchers.IO) {
                manager.setupPortForward(localPort, remotePort)
            }

            // Step 4: Start forwarding in background
            viewModelScope.launch(Dispatchers.IO) {
                portForward.start()
            }

            // Step 5: Update session with URL
            val session = sessions.value?.find { it.id == sessionId }
            if (session != null) {
                val updatedSession = session.copy(
                    isConnected = true,
                    localUrl = manager.getZellijUrl(session.zellijSessionName),
                    sshConfig = sshConfig
                )
                updateSession(updatedSession)
            }

            // Store manager
            sshManagers[sessionId] = manager

        } catch (e: Exception) {
            manager.disconnect()
            throw e
        }
    }

    /**
     * Disconnect a session
     */
    fun disconnectSession(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            sshManagers[sessionId]?.let { manager ->
                try {
                    manager.stopZellijWeb()
                } catch (e: Exception) {
                    // Log but continue
                }
                manager.disconnect()
                sshManagers.remove(sessionId)
            }

            // Update session state
            val session = sessions.value?.find { it.id == sessionId }
            if (session != null) {
                updateSession(session.copy(isConnected = false, localUrl = null))
            }
        }
    }

    private fun findAvailablePort(): Int {
        // Start from 9000 and find first available
        for (port in 9000..9100) {
            try {
                java.net.ServerSocket(port).use { return port }
            } catch (e: IOException) {
                // Port in use, try next
            }
        }
        throw IOException("No available ports in range 9000-9100")
    }

    override fun onCleared() {
        super.onCleared()
        // Clean up all SSH connections
        sshManagers.values.forEach { it.disconnect() }
        sshManagers.clear()
    }
}
```

### 5. Updated TerminalFragment

```kotlin
class TerminalFragment : Fragment() {
    private lateinit var webView: WebView
    private lateinit var viewModel: TerminalViewModel
    private var sessionId: String? = null

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        sessionId = arguments?.getString(ARG_SESSION_ID)
        viewModel = ViewModelProvider(requireActivity())[TerminalViewModel::class.java]

        webView = view.findViewById(R.id.terminalWebView)
        setupWebView()

        // Observe session for URL changes
        viewModel.sessions.observe(viewLifecycleOwner) { sessions ->
            val session = sessions.find { it.id == sessionId }
            session?.localUrl?.let { url ->
                if (webView.url != url) {
                    loadZellijWeb(url)
                }
            }
        }
    }

    private fun setupWebView() {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            allowContentAccess = true
            allowFileAccess = false
            mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW // For localhost
        }

        webView.webViewClient = object : WebViewClient() {
            override fun onReceivedError(
                view: WebView?,
                request: WebResourceRequest?,
                error: WebResourceError?
            ) {
                super.onReceivedError(view, request, error)
                // Handle connection errors (retry, show message, etc.)
            }
        }
    }

    private fun loadZellijWeb(url: String) {
        webView.loadUrl(url)
    }

    companion object {
        private const val ARG_SESSION_ID = "session_id"

        fun newInstance(sessionId: String) = TerminalFragment().apply {
            arguments = Bundle().apply {
                putString(ARG_SESSION_ID, sessionId)
            }
        }
    }
}
```

### 6. SSH Configuration UI

Create a dialog or activity for SSH connection setup:

```kotlin
data class SSHConnectionDialog(
    val onConnect: (SSHConfig) -> Unit
) {
    // Fields:
    // - Host (EditText)
    // - Port (EditText, default 22)
    // - Username (EditText)
    // - Authentication Method (RadioGroup: Password / Private Key)
    // - Password (EditText, shown if Password selected)
    // - Private Key Path (File picker, shown if Private Key selected)
    // - Key Passphrase (EditText, optional)
    // - Save Connection (CheckBox)
}
```

## Key Considerations

### 1. No Custom Keyboard Needed (Initially)
Since Zellij web provides its own web-based UI, the custom keyboard (ModBar/AlphaGrid) may not be necessary in the initial implementation. However, it could still be useful for:
- Local terminal sessions (future feature)
- Better mobile UX than web keyboard
- Offline mode support

### 2. Session Persistence
Store SSH connection details securely:
```kotlin
// Use Android Keystore for sensitive data
class SecureSSHStorage {
    fun saveSSHConfig(config: SSHConfig)
    fun loadSSHConfigs(): List<SSHConfig>
    fun deleteSSHConfig(host: String)
}
```

### 3. Network Permissions
```xml
<!-- AndroidManifest.xml -->
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
```

### 4. Background Service
Consider using a foreground service to keep SSH connections alive:
```kotlin
class SSHTunnelService : Service() {
    // Maintains SSH connections and port forwards
    // Shows persistent notification
}
```

### 5. Error Handling
- SSH connection failures
- Zellij web startup failures
- Port forwarding interruptions
- Network changes (WiFi ↔ mobile)
- Authentication errors

### 6. Security Best Practices
- **Host Key Verification**: Replace `PromiscuousVerifier` with proper known_hosts checking
- **Secure Storage**: Use Android Keystore for passwords/keys
- **HTTPS**: Ensure Zellij web uses HTTPS if exposed to network
- **Token Management**: Handle Zellij authentication tokens securely

## Revised Milestones

The implementation milestones need adjustment:

### New Milestone 1: SSH Connection Foundation
- Add SSHJ dependency
- Implement SSHConnectionManager
- Create SSH configuration UI
- Test basic SSH connection and command execution

### New Milestone 2: Zellij Web Integration
- Implement startZellijWeb() logic
- Test remote Zellij web server startup
- Verify Zellij web is accessible

### New Milestone 3: Port Forwarding
- Implement local port forwarding
- Handle port forwarding lifecycle
- Test tunnel stability

### New Milestone 4: WebView Integration
- Load Zellij web in WebView
- Handle authentication tokens
- Test full end-to-end flow

### Milestone 5-8: Remain Similar
- Multi-session support
- Session persistence
- Polish & optimization
- Advanced features

## Testing Considerations

### Unit Tests
- SSHConnectionManager logic
- Port allocation
- Session state management

### Integration Tests
- SSH connection (requires test server)
- Port forwarding functionality
- WebView loading Zellij web

### Manual Tests
- End-to-end: Connect → Start Zellij → Use Terminal
- Network interruption handling
- Multiple simultaneous sessions
- Session persistence across app restarts

## Advanced Integration: Gesture Controls & Session Persistence

### Gesture Integration with Zellij

Zellij supports keyboard shortcuts for tab navigation:
- **Alt+Left Arrow**: Previous tab
- **Alt+Right Arrow**: Next tab
- **Alt+H**: Previous tab (Vim-style)
- **Alt+L**: Next tab (Vim-style)

We want to map Android swipe gestures to these shortcuts for better mobile UX.

#### Dual-Layer Navigation

The app will have two navigation layers:

1. **Android ViewPager2 Level**: Swipe between different SSH hosts/connections
   - Each "page" is a different Zellij server connection
   - Full left/right swipe gesture

2. **Zellij Tab Level**: Swipe within terminal to navigate Zellij tabs
   - Gesture detected on WebView
   - Sends Alt+Arrow key sequences to Zellij
   - Short swipe or edge swipe

#### Implementation Strategy

```kotlin
class TerminalWebView(context: Context, attrs: AttributeSet) : WebView(context, attrs) {
    private var gestureDetector: GestureDetector
    private var onZellijTabSwipe: ((direction: SwipeDirection) -> Unit)? = null

    // Configuration
    private val MIN_SWIPE_DISTANCE = 100 // pixels
    private val MIN_SWIPE_VELOCITY = 100 // pixels/sec
    private val EDGE_SWIPE_MARGIN = 50 // pixels from edge

    init {
        gestureDetector = GestureDetector(context, SwipeGestureListener())
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        gestureDetector.onTouchEvent(event)
        return super.onTouchEvent(event)
    }

    private inner class SwipeGestureListener : GestureDetector.SimpleOnGestureListener() {
        override fun onFling(
            e1: MotionEvent?,
            e2: MotionEvent,
            velocityX: Float,
            velocityY: Float
        ): Boolean {
            if (e1 == null) return false

            val deltaX = e2.x - e1.x
            val deltaY = e2.y - e1.y

            // Horizontal swipe (not diagonal)
            if (abs(deltaX) > abs(deltaY) &&
                abs(deltaX) > MIN_SWIPE_DISTANCE &&
                abs(velocityX) > MIN_SWIPE_VELOCITY) {

                // Check if it's an edge swipe (for ViewPager navigation)
                val isLeftEdgeSwipe = e1.x < EDGE_SWIPE_MARGIN && deltaX > 0
                val isRightEdgeSwipe = e1.x > (width - EDGE_SWIPE_MARGIN) && deltaX < 0

                if (isLeftEdgeSwipe || isRightEdgeSwipe) {
                    // Let ViewPager2 handle edge swipes
                    return false
                }

                // Center swipe: send to Zellij
                val direction = if (deltaX > 0) SwipeDirection.RIGHT else SwipeDirection.LEFT
                onZellijTabSwipe?.invoke(direction)
                return true
            }

            return false
        }
    }

    fun setOnZellijTabSwipeListener(listener: (SwipeDirection) -> Unit) {
        onZellijTabSwipe = listener
    }
}

enum class SwipeDirection {
    LEFT, RIGHT
}
```

#### Sending Key Sequences to Zellij Web

Since we're loading Zellij in a WebView, we need to inject JavaScript to send key sequences:

```kotlin
class TerminalFragment : Fragment() {

    private fun setupGestureHandler() {
        terminalWebView.setOnZellijTabSwipeListener { direction ->
            sendZellijTabSwitch(direction)
        }
    }

    private fun sendZellijTabSwitch(direction: SwipeDirection) {
        val keyCode = when (direction) {
            SwipeDirection.LEFT -> "ArrowLeft"
            SwipeDirection.RIGHT -> "ArrowRight"
        }

        // Inject JavaScript to simulate Alt+Arrow keypress
        val js = """
            (function() {
                const event = new KeyboardEvent('keydown', {
                    key: '$keyCode',
                    code: '$keyCode',
                    altKey: true,
                    bubbles: true,
                    cancelable: true
                });
                document.dispatchEvent(event);

                // Also dispatch keyup
                const eventUp = new KeyboardEvent('keyup', {
                    key: '$keyCode',
                    code: '$keyCode',
                    altKey: true,
                    bubbles: true,
                    cancelable: true
                });
                document.dispatchEvent(eventUp);
            })();
        """.trimIndent()

        webView.evaluateJavascript(js, null)
    }
}
```

#### ViewPager2 Configuration

Disable swiping except at edges:

```kotlin
class MainActivity : AppCompatActivity() {

    private fun setupViewPager() {
        viewPager.isUserInputEnabled = true // Enable swipe

        // Reduce sensitivity so center swipes go to terminal
        viewPager.offscreenPageLimit = 1

        // Optional: Add custom touch interceptor
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                viewModel.setActiveSessionIndex(position)
            }
        })
    }
}
```

### Session Persistence in Zellij

One of Zellij's core features is session persistence. This works automatically:

#### How Zellij Sessions Work

1. **Session Lifecycle**:
   ```
   zellij attach my-session   # Creates or attaches to session
   # User disconnects
   # Session keeps running in background
   zellij attach my-session   # Reconnects to same session
   ```

2. **Web Client Session URLs**:
   - `http://localhost:8082/project-alpha` → Creates/attaches to "project-alpha"
   - WebSocket disconnects → Session remains active on server
   - Reload same URL → Reconnects to existing session

3. **Session Storage**:
   - Zellij serializes session metadata to disk
   - Sessions survive Zellij restarts (with `--session` flag)
   - Tabs, panes, and layouts are preserved

#### Android Implementation

```kotlin
data class TerminalSession(
    val id: String,
    val title: String,
    val sshConfig: SSHConfig? = null,

    // Persistent Zellij session name
    // This is the key to reconnecting to the same session
    val zellijSessionName: String,

    val isConnected: Boolean = false,
    val localUrl: String? = null,

    // Track last connection time
    val lastConnected: Long = System.currentTimeMillis()
)

class TerminalViewModel : ViewModel() {

    /**
     * Reconnect to existing Zellij session
     * Uses the same zellijSessionName to attach to running session
     */
    suspend fun reconnectSession(sessionId: String) {
        val session = sessions.value?.find { it.id == sessionId }
            ?: throw IllegalArgumentException("Session not found")

        val sshConfig = session.sshConfig
            ?: throw IllegalStateException("No SSH config for session")

        // Reuse existing session name - this is the key!
        // Zellij will attach to the existing session instead of creating new
        connectSession(sessionId, sshConfig)

        // The session should still have all its tabs and state
    }

    /**
     * Disconnect from Android side only
     * Zellij session continues running on server
     */
    fun softDisconnect(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            sshManagers[sessionId]?.let { manager ->
                // Close port forward and SSH, but DON'T stop Zellij
                // (don't call manager.stopZellijWeb())
                manager.disconnect()
                sshManagers.remove(sessionId)
            }

            val session = sessions.value?.find { it.id == sessionId }
            if (session != null) {
                updateSession(session.copy(
                    isConnected = false,
                    localUrl = null,
                    lastConnected = System.currentTimeMillis()
                ))
            }
        }
    }

    /**
     * Hard disconnect - stops Zellij session on server
     * Use this only when explicitly closing a session
     */
    fun hardDisconnect(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            sshManagers[sessionId]?.let { manager ->
                try {
                    manager.stopZellijWeb()
                } catch (e: Exception) {
                    // Log error
                }
                manager.disconnect()
                sshManagers.remove(sessionId)
            }

            // Remove session from list
            val updatedSessions = sessions.value?.filter { it.id != sessionId }
            sessions.postValue(updatedSessions)
        }
    }
}
```

#### Session Persistence UI

```kotlin
class SessionListActivity : AppCompatActivity() {

    /**
     * Show list of saved sessions with their connection status
     */
    private fun displaySessions() {
        // Sessions can be:
        // 1. Connected (green): SSH connected, actively viewing
        // 2. Running (yellow): SSH disconnected, but Zellij running on server
        // 3. Stopped (gray): Zellij session killed, can be resurrected

        recyclerView.adapter = SessionAdapter(
            sessions = viewModel.sessions.value ?: emptyList(),
            onConnect = { session ->
                if (session.isConnected) {
                    // Jump to session in ViewPager
                    navigateToSession(session)
                } else {
                    // Reconnect to existing Zellij session
                    viewModel.reconnectSession(session.id)
                }
            },
            onDisconnect = { session ->
                // Soft disconnect - session keeps running
                viewModel.softDisconnect(session.id)
            },
            onDelete = { session ->
                // Hard disconnect - kills Zellij session
                showConfirmDialog("Kill session and all running processes?") {
                    viewModel.hardDisconnect(session.id)
                }
            }
        )
    }
}
```

### Lifecycle Management

Handle Android lifecycle events properly:

```kotlin
class MainActivity : AppCompatActivity() {

    override fun onPause() {
        super.onPause()
        // Soft disconnect all sessions when app goes to background
        // Sessions keep running on servers
        viewModel.softDisconnectAll()
    }

    override fun onResume() {
        super.onResume()
        // Optionally auto-reconnect to last active session
        viewModel.autoReconnect()
    }

    override fun onDestroy() {
        super.onDestroy()
        if (isFinishing) {
            // App is closing, not just rotating
            // Decide: soft disconnect (default) or hard disconnect
            viewModel.softDisconnectAll()
        }
    }
}
```

### Additional Gesture Considerations

#### Three-Finger Swipe for Special Functions

```kotlin
private inner class MultiTouchGestureListener : View.OnTouchListener {
    override fun onTouch(v: View?, event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.pointerCount == 3) {
                    // Three-finger swipe detected
                    // Could trigger: new tab, split pane, etc.
                }
            }
        }
        return false
    }
}
```

#### Long Press for Context Menu

```kotlin
terminalWebView.setOnLongClickListener {
    showZellijContextMenu()
    true
}

private fun showZellijContextMenu() {
    PopupMenu(requireContext(), terminalWebView).apply {
        menu.add("New Tab")
        menu.add("Split Pane")
        menu.add("Close Tab")
        menu.add("Rename Session")

        setOnMenuItemClickListener { item ->
            when (item.title) {
                "New Tab" -> sendZellijCommand("Ctrl+t")
                "Split Pane" -> sendZellijCommand("Ctrl+p, d")
                // etc.
            }
            true
        }
    }.show()
}
```

### Configuration

Add settings for gesture sensitivity:

```kotlin
data class GestureConfig(
    val enableZellijTabSwipe: Boolean = true,
    val swipeSensitivity: Float = 1.0f, // 0.5 - 2.0
    val edgeSwipeMargin: Int = 50, // pixels
    val preferZellijNavigation: Boolean = true // vs ViewPager navigation
)
```

## References

- [Zellij Web Client Documentation](https://zellij.dev/documentation/web-client.html)
- [Building Zellij's Web Client](https://poor.dev/blog/building-zellij-web-terminal/)
- [SSHJ GitHub Repository](https://github.com/hierynomus/sshj)
- [SSHJ Library Guide](https://www.javaguides.net/2024/05/guide-to-sshj-library-in-java.html)
- [Apache MINA SSHD Port Forwarding](https://github.com/apache/mina-sshd/blob/master/docs/port-forwarding.md)
