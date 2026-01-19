# Zelland Architecture with Tailscale

## Overview

Zelland uses **Tailscale** for network connectivity, allowing direct access to remote Zellij web servers without SSH port forwarding.

## Revised Architecture

```
┌─────────────────────────────────────────────────┐
│              Android Device                      │
│            (Tailscale Client)                    │
│  ┌───────────────────────────────────────────┐  │
│  │          WebView (Zellij Web)             │  │
│  │   http://<tailscale-ip>:8082/session     │  │
│  └───────────────────────────────────────────┘  │
│                      ↓↑                          │
│  ┌───────────────────────────────────────────┐  │
│  │        Direct Connection                   │  │
│  │     (via Tailscale network)                │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
│  ┌───────────────────────────────────────────┐  │
│  │     SSH (for control only)                 │  │
│  │   - Start/stop Zellij                      │  │
│  │   - Run commands                           │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
                       ↓↑
              (Tailscale Network)
                       ↓↑
┌─────────────────────────────────────────────────┐
│           Remote Host (Tailscale Node)          │
│  ┌───────────────────────────────────────────┐  │
│  │      Zellij Web Server (port 8082)        │  │
│  │    Listening on Tailscale interface       │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## Key Changes from Original Design

### What We Keep ✅
- SSH for control operations (start/stop Zellij, execute commands)
- SSHConnectionManager
- ZellijManager
- All session management logic
- TerminalViewModel
- SSH authentication (password/key)

### What We Remove ❌
- **No SSH port forwarding** (LocalPortForwarder)
- **No PortForwardHandle**
- **No findAvailablePort()**
- **No localhost:9000 URLs**

### What We Add ➕
- **Direct Tailscale IP connectivity**
- **Zellij web must bind to Tailscale interface**
- **WebView loads remote URL directly**

## Connection Flow

### 1. Prerequisites
```
User has:
- Tailscale installed on Android device
- Tailscale installed on remote host
- Both devices connected to same Tailnet
```

### 2. Session Connection
```kotlin
fun connectSession(sessionId: String) {
    // 1. Connect via SSH (over Tailscale)
    sshManager.connect(sshConfig)

    // 2. Get Tailscale IP of remote host
    val tailscaleIP = getTailscaleIP()

    // 3. Start Zellij web (bind to Tailscale interface)
    zellijManager.startZellijWeb()

    // 4. Build direct URL
    val zellijUrl = "http://$tailscaleIP:8082/${session.zellijSessionName}"

    // 5. Load in WebView
    webView.loadUrl(zellijUrl)
}
```

### 3. WebView Display
```kotlin
// In TerminalFragment
webView.loadUrl("http://100.64.1.5:8082/my-session")
// Tailscale handles routing!
```

## Tailscale Integration

### Getting Tailscale IP

**Option 1: Use SSH config host**
```kotlin
// If user enters Tailscale hostname in SSH config:
// Host: myserver.tailnet-abc.ts.net
// Port: 22
// Then Zellij URL: http://myserver.tailnet-abc.ts.net:8082/session
val zellijUrl = "http://${sshConfig.host}:8082/${sessionName}"
```

**Option 2: Query Tailscale IP via SSH**
```kotlin
suspend fun getTailscaleIP(): String {
    val result = sshManager.executeCommand("tailscale ip -4")
    return result.stdout.trim()  // e.g., "100.64.1.5"
}
```

**Option 3: User provides Tailscale IP**
```kotlin
// Add field to SSHConfig:
data class SSHConfig(
    // ...existing fields...
    val tailscaleIP: String? = null  // Optional override
)

// Use in URL:
val ip = sshConfig.tailscaleIP ?: sshConfig.host
val zellijUrl = "http://$ip:8082/${sessionName}"
```

### Ensuring Zellij Binds to Tailscale

By default, Zellij web may bind to localhost only. We need to ensure it listens on the Tailscale interface.

**Option 1: Check Zellij configuration**
```bash
# On remote host:
cat ~/.config/zellij/config.kdl
# Ensure web server binds to 0.0.0.0 or Tailscale IP
```

**Option 2: Use `--bind` flag (if available)**
```kotlin
// In ZellijManager.startZellijWeb()
val startCommand = """
    nohup zellij web --bind 0.0.0.0:8082 > /tmp/zellij-web.log 2>&1 &
    echo $!
""".trimIndent()
```

**Option 3: SSH tunnel just for control (optional fallback)**
If Zellij doesn't support binding to specific interface, we could still use SSH tunnel, but since you have Tailscale, this shouldn't be needed.

## Security Considerations

### Tailscale Network Security
- ✅ Tailscale provides encrypted connections (WireGuard)
- ✅ Only devices in your Tailnet can access
- ✅ Zellij web listens on Tailscale interface only (not public internet)

### Zellij Web Authentication
From [Zellij docs](https://zellij.dev/documentation/web-client.html):
> Zellij requires any user be authenticated with a special token before they can log in from the browser

**Implementation needed**:
```kotlin
// In ZellijManager, after starting web server:
suspend fun getZellijWebToken(): String {
    // Option 1: Parse from Zellij output/logs
    // Option 2: Read from Zellij config file
    // Option 3: User provides manually

    val result = sshManager.executeCommand(
        "cat ~/.config/zellij/web-tokens.kdl 2>/dev/null || echo ''"
    )
    return parseToken(result.stdout)
}

// Then in WebView:
val urlWithToken = "http://$ip:8082/$session?token=$token"
webView.loadUrl(urlWithToken)
```

### Network Security Config
Android requires cleartext traffic to be allowed for HTTP:

```xml
<!-- res/xml/network_security_config.xml -->
<network-security-config>
    <!-- Allow HTTP to Tailscale IP range -->
    <domain-config cleartextTrafficPermitted="true">
        <!-- Tailscale uses 100.64.0.0/10 -->
        <domain includeSubdomains="true">100.64.*.*</domain>
        <!-- Or allow Tailscale hostnames -->
        <domain includeSubdomains="true">*.ts.net</domain>
    </domain-config>
</network-security-config>
```

**Better**: Use HTTPS with Tailscale HTTPS (if enabled)
```kotlin
val zellijUrl = "https://${sshConfig.host}:8082/${sessionName}"
// Tailscale can provide automatic HTTPS for *.ts.net domains
```

## Updated Milestones

### ~~Milestone 3: SSH Port Forwarding~~ → SKIP! ✅

We can go directly to WebView integration!

### New Milestone 3: WebView Integration (Tailscale)

**Goal**: Load Zellij web in WebView using direct Tailscale connection

**Tasks**:
1. Determine Tailscale IP of remote host
2. Ensure Zellij web binds to Tailscale interface
3. Handle Zellij authentication tokens
4. Create TerminalFragment with WebView
5. Load Zellij web URL
6. Test connectivity
7. Handle WebView lifecycle

**Estimated Time**: 3-4 hours

### Milestone 4: Gesture Controls
(Unchanged - still need swipe-to-tab functionality)

### Milestone 5: Multi-Session with ViewPager2
(Simplified - no port forwarding to manage per session)

## Implementation Changes

### SSHConfig Updates

```kotlin
data class SSHConfig(
    // ... existing fields ...

    // NEW: Optional Tailscale hostname/IP
    val tailscaleHost: String? = null,

    // Use regular host for SSH, tailscaleHost for WebView
    // Example:
    //   host = "myserver.tailnet-abc.ts.net" (used for both)
    //   OR
    //   host = "ssh.example.com" (SSH only)
    //   tailscaleHost = "myserver.ts.net" (WebView only)
)
```

### TerminalSession Updates

```kotlin
data class TerminalSession(
    // ... existing fields ...

    // Direct URL to Zellij web (no localhost)
    val zellijWebUrl: String? = null,
    // Example: "http://100.64.1.5:8082/session-abc123"
)
```

### ZellijManager Updates

```kotlin
class ZellijManager {
    // Add method to get Tailscale IP
    suspend fun getTailscaleIP(): String? {
        try {
            val result = sshManager.executeCommand("tailscale ip -4")
            if (result.success) {
                return result.stdout.trim()
            }
        } catch (e: Exception) {
            return null
        }
        return null
    }

    // Update startZellijWeb to bind to 0.0.0.0
    suspend fun startZellijWeb(bindAddress: String = "0.0.0.0"): Int {
        val startCommand = """
            nohup zellij web --bind $bindAddress:8082 > /tmp/zellij-web.log 2>&1 &
            echo $!
        """.trimIndent()
        // ... rest of implementation
    }
}
```

### TerminalViewModel Updates

```kotlin
fun connectSession(sessionId: String) {
    viewModelScope.launch {
        // ... SSH connection and Zellij startup ...

        // Get Tailscale IP
        val tailscaleIP = zellijManager.getTailscaleIP()
            ?: sshConfig.tailscaleHost
            ?: sshConfig.host

        // Build Zellij web URL
        val zellijUrl = "http://$tailscaleIP:8082/${session.zellijSessionName}"

        // Update session with URL
        updateSession(session.copy(
            isConnected = true,
            zellijWebUrl = zellijUrl
        ))
    }
}
```

## Testing with Tailscale

### Setup
```bash
# 1. Install Tailscale on Android
# Download from Play Store: https://play.google.com/store/apps/details?id=com.tailscale.ipn

# 2. Install Tailscale on remote host
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up

# 3. Get Tailscale IP
tailscale ip -4
# Example output: 100.64.1.5

# 4. Start Zellij web bound to Tailscale
zellij web --bind 0.0.0.0:8082
```

### Verify Connectivity
```bash
# On Android device (via Termux or adb shell):
curl http://100.64.1.5:8082
# Should return Zellij web HTML
```

## Benefits of Tailscale Approach

1. **Simpler**: No port forwarding code needed
2. **More Reliable**: Direct connection, no tunnel overhead
3. **Better Performance**: No SSH tunnel latency
4. **More Flexible**: Works even when SSH server is on different port/host
5. **Easier Testing**: Can test Zellij web directly in browser first
6. **Works Offline**: If both devices on same LAN + Tailscale

## Drawbacks

1. **Requires Tailscale**: User must set up Tailscale on both ends
2. **Network Dependency**: Need active Tailnet connection
3. **No Fallback**: If Tailscale fails, entire app fails
   - **Mitigation**: Could add SSH tunnel as fallback option in settings

## Recommended Approach

**For MVP**: Use Tailscale exclusively
- Simpler implementation
- Document Tailscale requirement
- Faster to market

**For Future**: Add SSH tunnel as fallback
- Settings toggle: "Use Tailscale" vs "Use SSH tunnel"
- Auto-detect which method to use
- Best of both worlds

## Next Steps

1. ✅ Update documentation with Tailscale architecture
2. ⏭️ Skip Milestone 3 (port forwarding)
3. → Jump to WebView integration (new Milestone 3)
4. Add Tailscale IP detection
5. Test end-to-end with real Tailscale network

Ready to implement the WebView integration? 🚀
