# Testing Milestone 3: WebView Integration with Tailscale

## Quick Test Guide

### Prerequisites

1. **Tailscale Setup** (both devices):
   ```bash
   # On Android device:
   # - Install Tailscale from Play Store
   # - Sign in and connect to your Tailnet

   # On remote host:
   curl -fsSL https://tailscale.com/install.sh | sh
   sudo tailscale up
   tailscale ip -4  # Note the IP (e.g., 100.64.1.5)
   ```

2. **Zellij Installation** (remote host):
   ```bash
   curl -L zellij.dev/install.sh | bash
   zellij --version  # Verify 0.43.0+
   ```

3. **Verify Connectivity**:
   ```bash
   # On Android (via Termux or adb shell):
   ping {remote-tailscale-ip}
   # Should get responses
   ```

### Test 1: Basic WebView Display

**Goal**: Verify Zellij web loads in WebView over Tailscale

1. Open Zelland app
2. Add SSH connection for Tailscale-connected host
3. Tap "Connect" on session card
4. **Watch status updates**:
   - ✅ "Connecting to host..."
   - ✅ "Checking for Zellij..."
   - ✅ "Starting Zellij web server..."
   - ✅ "Getting Tailscale IP..."
   - ✅ "Getting authentication token..."
   - ✅ "Connected to [session]"
5. **Verify WebView**:
   - Full-screen terminal displays
   - Zellij web interface is visible
   - Status bar at bottom shows Zellij info
   - Progress bar disappears after load

**Expected**: Zellij terminal loads and is interactive

**Logs to check**:
```bash
adb logcat -s TerminalViewModel:I ZellijManager:D TerminalFragment:*

# Should see:
I/TerminalViewModel: Tailscale IP: 100.64.1.5
D/ZellijManager: Generated Zellij auth token
I/TerminalViewModel: Zellij URL: http://100.64.1.5:8082/session-...
D/TerminalFragment: Page loaded: http://100.64.1.5:8082/...
```

---

### Test 2: Terminal Interaction

**Goal**: Verify terminal is fully interactive

1. With Zellij web loaded in WebView
2. **Try typing commands**:
   ```bash
   ls -la
   pwd
   echo "Hello from Zelland"
   ```
3. **Verify**:
   - Commands execute and show output
   - Terminal scrolls properly
   - Backspace/delete works
   - Enter key submits commands

**Expected**: All terminal input/output works correctly

---

### Test 3: Tailscale IP Detection

**Goal**: Verify app gets correct Tailscale IP

1. **On remote host**, check Tailscale IP:
   ```bash
   tailscale ip -4
   # Note the IP (e.g., 100.64.1.5)
   ```

2. **In app**, connect to session

3. **Check logs** for Tailscale IP:
   ```bash
   adb logcat -s TerminalViewModel:* | grep "Tailscale IP"

   # Should see:
   I/TerminalViewModel: Tailscale IP: 100.64.1.5
   ```

4. **Verify** IP matches what `tailscale ip -4` returned

**Expected**: App detects correct Tailscale IP

---

### Test 4: Authentication Token

**Goal**: Verify token creation and usage

1. Connect to session
2. **Check logs** for token creation:
   ```bash
   adb logcat -s ZellijManager:D TerminalViewModel:*

   # Should see:
   D/TerminalViewModel: Got Zellij auth token
   ```

3. **Verify URL includes token** (in logs):
   ```
   I/TerminalViewModel: Zellij URL: http://100.64.1.5:8082/session-...
   ```
   (Note: Token in URL is not logged for security)

4. **Verify access** - WebView loads successfully

**Expected**: Token is created and used in URL

**Manual verification** (optional):
```bash
# On remote host:
zellij web --create-token
# Should output a token string

# Try loading in browser:
# http://{tailscale-ip}:8082/session?token={token}
# Should work without additional auth
```

---

### Test 5: Reuse Existing Zellij Web

**Goal**: Verify app doesn't start duplicate servers

1. **On remote host**, start Zellij web manually:
   ```bash
   zellij web &
   pgrep -f 'zellij web'  # Note the PID
   ```

2. **In app**, connect to session

3. **Check logs**:
   ```bash
   adb logcat -s ZellijManager:I

   # Should see:
   I/ZellijManager: Zellij web already running
   ```

4. **On remote host**, verify only one process:
   ```bash
   pgrep -f 'zellij web'  # Should show same PID
   ```

**Expected**: App reuses existing Zellij web server

---

### Test 6: Back Navigation

**Goal**: Verify back button returns to session list

1. With Zellij web loaded in WebView
2. Press back button
3. **Verify**:
   - Returns to session list
   - Session shows "Disconnected"
   - FAB is visible again
   - Fragment container is hidden

4. **On remote host**, verify Zellij still running:
   ```bash
   pgrep -f 'zellij web'  # Should still show PID
   ```

**Expected**: Soft disconnect - UI returns to list, Zellij stays alive

---

### Test 7: Reconnection

**Goal**: Verify reconnecting to same session works

1. Connect to session (Zellij loads in WebView)
2. Press back button (return to session list)
3. **Immediately** tap "Connect" again
4. **Verify**:
   - Reconnects quickly (< 2 seconds)
   - Same Zellij session restored
   - Terminal state preserved

5. **Check logs** for reuse:
   ```bash
   adb logcat -s ZellijManager:I

   # Should see:
   I/ZellijManager: Zellij web already running
   ```

**Expected**: Fast reconnection, session state preserved

---

### Test 8: Multiple Sessions

**Goal**: Verify multiple sessions work independently

1. Add 2-3 SSH connections (different hosts or users)
2. Connect to first session
3. Press back, connect to second session
4. **Verify**:
   - Each session connects independently
   - Each gets its own Zellij instance
   - Switching between them works smoothly

**Expected**: Multiple sessions work in parallel

---

### Test 9: Error - Tailscale Not Connected

**Goal**: Verify graceful error when Tailscale unavailable

1. **On Android**, disconnect from Tailscale:
   - Open Tailscale app
   - Tap "Disconnect"

2. **In Zelland**, try to connect to session

3. **Verify error handling**:
   - Connection fails after timeout
   - WebView shows error toast
   - Status shows error message

**Expected**: Clear error message about Tailscale connectivity

**Recovery**:
- Reconnect to Tailscale
- Try connecting again - should work

---

### Test 10: Error - Zellij Web Fails to Start

**Goal**: Verify error handling when Zellij web fails

1. **On remote host**, block port 8082:
   ```bash
   # Start something else on port 8082
   python3 -m http.server 8082 &
   ```

2. **In app**, try to connect

3. **Verify error message**:
   - Shows "Zellij error: Failed to start"
   - Check logs for detailed error

4. **Clean up**:
   ```bash
   pkill -f "http.server"
   ```

**Expected**: Helpful error message, graceful failure

---

## Performance Tests

### WebView Load Time

**Goal**: Zellij web should load within 3-5 seconds

1. Connect to session
2. Time from "Connected" status to WebView displaying terminal
3. **Acceptable**: 2-5 seconds
4. **Slow**: > 10 seconds (investigate network/server)

### Memory Usage

**Goal**: WebView shouldn't leak memory

1. Connect to session (WebView loads)
2. Press back (disconnect)
3. Repeat 5-10 times
4. **Check memory** with Android Profiler
5. **Expected**: Memory usage stable, no continuous growth

---

## Debug Commands

### On Remote Host

```bash
# Check Tailscale IP:
tailscale ip -4

# Check Tailscale status:
tailscale status

# Check if Zellij web running:
pgrep -f 'zellij web'

# Check what's on port 8082:
lsof -i :8082

# View Zellij logs:
tail -f /tmp/zellij-web.log

# Test Zellij web manually:
curl http://localhost:8082

# Create token manually:
zellij web --create-token

# Kill Zellij web:
pkill -f 'zellij web'
```

### On Android

```bash
# Watch all Zelland logs:
adb logcat -s ZellijManager:* TerminalViewModel:* TerminalFragment:* MainActivity:*

# Watch network errors:
adb logcat | grep -i "webview\|connection"

# Test Tailscale connectivity (via Termux):
ping {remote-tailscale-ip}
curl http://{remote-tailscale-ip}:8082

# Check app process:
adb shell ps | grep zelland
```

---

## Common Issues

### Issue: "Failed to load Zellij web" in WebView

**Symptoms**:
- WebView shows connection error
- Toast says "Connection error. Check if Tailscale is connected."

**Causes**:
1. Tailscale not connected on Android
2. Remote host offline or not on Tailnet
3. Zellij web not listening on Tailscale interface

**Debug**:
```bash
# Check Tailscale status on Android:
# Open Tailscale app, verify "Connected"

# Check connectivity:
ping {remote-tailscale-ip}

# Check Zellij web is accessible:
curl http://{remote-tailscale-ip}:8082
```

**Solution**:
- Ensure Tailscale connected on both devices
- Verify both devices on same Tailnet
- Check Zellij web is running: `pgrep -f 'zellij web'`

---

### Issue: Blank WebView, No Error

**Symptoms**:
- WebView loads (progress bar completes)
- But shows blank screen
- No error message

**Causes**:
1. Authentication token invalid
2. Zellij web requires specific session name
3. JavaScript disabled (shouldn't happen with our config)

**Debug**:
```bash
# Check WebView logs:
adb logcat -s chromium:* WebView:*

# Check if token was created:
adb logcat -s ZellijManager:D | grep token

# Try loading in browser on Android:
# http://{tailscale-ip}:8082/session-{id}?token={token}
```

**Solution**:
- Restart Zellij web: `pkill -f 'zellij web' && zellij web &`
- Check Zellij version is 0.43.0+
- Verify token creation works manually: `zellij web --create-token`

---

### Issue: "Mixed Content Blocked" Error

**Symptoms**:
- WebView console shows mixed content errors
- Resources fail to load

**Cause**: WebView blocking HTTP content

**Solution**: Should already be fixed in code, but verify:

1. Check `TerminalFragment.kt` has:
   ```kotlin
   mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
   ```

2. Check `network_security_config.xml` has:
   ```xml
   <domain-config cleartextTrafficPermitted="true">
       <domain includeSubdomains="true">100.64.*.*</domain>
   </domain-config>
   ```

---

### Issue: Back Button Doesn't Work

**Symptoms**:
- Pressing back exits app instead of returning to session list

**Cause**: `onBackPressed()` not handling fragment correctly

**Debug**:
```bash
# Check logs:
adb logcat -s MainActivity:*

# Should see logic for back handling
```

**Solution**: Verify `MainActivity.onBackPressed()` is implemented correctly

---

## Success Criteria

**Milestone 3 is successful if**:

1. ✅ Zellij web loads in WebView over Tailscale
2. ✅ Terminal is fully interactive (input/output works)
3. ✅ Tailscale IP is detected correctly
4. ✅ Authentication tokens are created and used
5. ✅ Existing Zellij web servers are reused
6. ✅ Back button returns to session list (soft disconnect)
7. ✅ Reconnection is fast (< 2 seconds)
8. ✅ Multiple sessions work independently
9. ✅ Error handling is graceful and informative
10. ✅ WebView lifecycle is properly managed

**All criteria met?** → Proceed to Milestone 4! 🎉

---

## Next: Milestone 4 Preview

In Milestone 4, we'll add gesture controls for Zellij tab navigation:

```
Swipe Left  → Send Alt+Left  → Previous Zellij tab
Swipe Right → Send Alt+Right → Next Zellij tab
```

This will allow seamless tab switching in Zellij without needing a keyboard.

**Ready to test?** Follow the steps above!
