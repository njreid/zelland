# Testing Milestone 2: Zellij Web Server Management

## Quick Test Guide

### Prerequisites

1. **Remote SSH Server** with:
   - SSH access (from Milestone 1)
   - [Zellij 0.43.0+](https://zellij.dev) installed
   ```bash
   # On your server:
   curl -L zellij.dev/install.sh | bash
   zellij --version  # Verify 0.43.0+
   ```

2. **Android Device/Emulator**
   - Zelland app installed
   - Network connectivity to SSH server

### Test 1: Basic Connection Flow

**Goal**: Verify complete SSH → Zellij startup flow

1. Open Zelland app
2. Tap "+" to add connection (if not already done)
3. Fill in SSH details for server with Zellij
4. Tap "Save"
5. **NEW**: Session card appears in list
6. **NEW**: Tap "Connect" button
7. **Watch status updates** (bottom of screen):
   - ✅ "Connecting to host..."
   - ✅ "Checking for Zellij..."
   - ✅ "Starting Zellij web server..."
   - ✅ "Connected to [session] (Zellij on port 8082)"
8. **Verify**:
   - Session card shows "Connected" (green)
   - "Disconnect" button now visible
   - "Connect" button hidden

**Expected**: Full connection succeeds, Zellij web starts

---

### Test 2: Zellij Already Running

**Goal**: Verify app detects existing Zellij web process

1. **On remote server**, start Zellij web manually:
   ```bash
   zellij web &
   pgrep -f 'zellij web'  # Note the PID
   ```

2. **In app**, tap "Connect" on the session

3. **Watch logs**:
   ```bash
   adb logcat -s ZellijManager:I
   # Should see: "Zellij web already running"
   ```

4. **Verify**:
   - Connection succeeds quickly (no startup delay)
   - Only one Zellij web process running (check with `pgrep`)

**Expected**: App reuses existing server, doesn't start duplicate

---

### Test 3: Disconnect (Soft)

**Goal**: Verify soft disconnect keeps Zellij running

1. With a connected session, tap "Disconnect"

2. **On remote server**, verify Zellij still running:
   ```bash
   pgrep -f 'zellij web'  # Should still return PID
   ```

3. **In app**:
   - Session shows "Disconnected" (gray)
   - "Connect" button reappears
   - "Disconnect" button hidden

4. **Reconnect** by tapping "Connect" again

5. **Verify**:
   - Reconnects quickly
   - Same Zellij process (same PID)

**Expected**: SSH closes, Zellij stays alive, can reconnect instantly

---

### Test 4: Error - Zellij Not Installed

**Goal**: Verify helpful error when Zellij missing

1. **On remote server**, temporarily hide Zellij:
   ```bash
   which zellij  # Note the path
   sudo mv /usr/local/bin/zellij /usr/local/bin/zellij.hidden
   # Or: mv ~/.local/bin/zellij ~/.local/bin/zellij.hidden
   ```

2. **In app**, try to connect

3. **Verify error message**:
   - Shows: "Zellij not found on [hostname]"
   - Status bar is red
   - Session remains "Disconnected"

4. **Restore Zellij**:
   ```bash
   sudo mv /usr/local/bin/zellij.hidden /usr/local/bin/zellij
   ```

5. **Reconnect** - should now work

**Expected**: Clear error message guides user to install Zellij

---

### Test 5: Multiple Sessions

**Goal**: Verify multiple sessions with independent Zellij servers

1. Add 2-3 different SSH connections
   - Different hosts, OR
   - Same host with different users/credentials

2. Tap "Connect" on first session
3. **While first is connecting**, tap "Connect" on second session

4. **Verify**:
   - Both sessions connect independently
   - Both show "Connected" when done
   - Status updates for most recent connection

5. **Disconnect one** session

6. **Verify**:
   - Other session stays connected
   - Can disconnect/reconnect each independently

**Expected**: Sessions are fully independent

---

### Test 6: View Logs

**Goal**: Check that logging works for debugging

1. Connect to a session

2. **Check Android logs**:
   ```bash
   adb logcat -s ZellijManager:* TerminalViewModel:*
   ```

3. **Expected log messages**:
   ```
   I/TerminalViewModel: Zellij version: 0.43.1
   D/ZellijManager: Started Zellij web with PID: 12345
   I/ZellijManager: Zellij web started successfully
   I/TerminalViewModel: Zellij web started on port 8082
   ```

4. **On connection failure**, logs should show:
   ```
   E/TerminalViewModel: Zellij error: [error details]
   ```

**Expected**: Detailed logs help diagnose issues

---

### Test 7: Zellij Logs Retrieval

**Goal**: Verify `getLogs()` can fetch remote Zellij logs

This feature is available but not exposed in UI yet. Test programmatically or via debug console.

**Future enhancement**: Add "View Logs" button to session card (Milestone 9)

---

## Performance Tests

### Startup Time

**Goal**: Zellij web should start within 5 seconds

1. Ensure Zellij web is NOT running on server
2. Time from "Connect" tap to "Connected" status
3. **Acceptable**: 2-5 seconds (including SSH handshake)
4. **Slow**: > 10 seconds (investigate network/server)

### Reconnection Time

**Goal**: Reconnecting to existing Zellij web should be instant

1. Connect, then disconnect
2. Time from second "Connect" to "Connected"
3. **Acceptable**: < 1 second
4. **Slow**: > 2 seconds (check if server is restarting Zellij)

---

## Debug Commands

### On Remote Server

```bash
# Check if Zellij web is running:
pgrep -f 'zellij web'

# View Zellij web logs:
tail -f /tmp/zellij-web.log

# Check what's listening on port 8082:
lsof -i :8082

# Manually start Zellij web:
zellij web

# Kill Zellij web:
pkill -f 'zellij web'

# List Zellij sessions:
zellij list-sessions

# Kill all Zellij sessions:
pkill zellij
```

### On Android

```bash
# Filter for Zelland logs:
adb logcat -s ZellijManager:* TerminalViewModel:* SSHConnectionManager:*

# Clear logs:
adb logcat -c

# Watch in real-time:
adb logcat | grep -i zellij

# Check app is running:
adb shell ps | grep zelland
```

---

## Known Issues & Workarounds

### Issue: "Startup failed" even though Zellij works manually

**Debug**:
```bash
# On server, check what command app runs:
adb logcat | grep "zellij web"

# Try running the exact command manually:
nohup zellij web > /tmp/zellij-web.log 2>&1 &

# Check logs immediately:
tail /tmp/zellij-web.log
```

**Common causes**:
- Port 8082 already in use
- Insufficient permissions
- Zellij config error

### Issue: Connection succeeds but status shows "Error"

**Debug**:
- Check logcat for exception details
- Verify ViewModel's exception handling didn't swallow useful info

**Workaround**:
- Add more granular status updates
- Surface more error details in UI

### Issue: "Disconnect" doesn't work

**Debug**:
```bash
# Verify SSH connection closes:
adb logcat -s SSHConnectionManager:*

# Check server:
who  # Should show user disconnect
```

---

## Success Criteria

**Milestone 2 is successful if**:

1. ✅ Can detect Zellij installation (or lack thereof)
2. ✅ Can start Zellij web server on remote host
3. ✅ Can detect if Zellij web already running
4. ✅ Can soft-disconnect (keep Zellij alive)
5. ✅ Can reconnect to existing Zellij web
6. ✅ Shows clear status updates during connection
7. ✅ Shows helpful error messages on failure
8. ✅ Multiple sessions work independently

**All criteria met?** → Proceed to Milestone 3! 🎉

---

## Next: Milestone 3 Preview

In Milestone 3, we'll add port forwarding so we can actually access the Zellij web UI from Android:

```
┌──────────────┐         ┌─────────────┐         ┌──────────────┐
│   Android    │ tunnel  │     SSH     │ forward │ Zellij Web   │
│ localhost:   │◄────────│             │────────►│  (port 8082) │
│    9000      │         │             │         │              │
└──────────────┘         └─────────────┘         └──────────────┘
```

**Ready to test?** Follow the steps above!

**Found a bug?** Check [MILESTONE_2_COMPLETE.md](MILESTONE_2_COMPLETE.md) for troubleshooting.
