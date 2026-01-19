# Zelland - Quick Start Guide

Get Zelland running in 5 minutes!

## Prerequisites

- **Android Studio** installed (Hedgehog 2023.1.1+)
- **Android device or emulator** (API 24+)
- **SSH server** with access credentials (for testing)

Optional:
- Remote server with [Zellij 0.43.0+](https://zellij.dev) installed

## Step 1: Open Project

```bash
cd /home/njr/code/droid

# Or rename to match branding:
mv /home/njr/code/droid /home/njr/code/zelland
cd /home/njr/code/zelland
```

Open in Android Studio:
```
File → Open → Select project directory
```

Wait for Gradle sync (first time: ~2-5 minutes).

## Step 2: Build & Run

### Option A: Android Studio (Recommended)

1. **Connect device** or **start emulator**
2. Click **Run** button (▶️) or press `Shift+F10`

### Option B: Command Line

```bash
# Build debug APK
./gradlew assembleDebug

# Install on device
adb install app/build/outputs/apk/debug/app-debug.apk
```

## Step 3: Test SSH Connection

### What You'll See

1. **Empty State Screen**
   - "Zelland" logo
   - "No sessions yet" message
   - "Add Session" button

2. **Tap "Add Session" (+)**

3. **Fill SSH Configuration Form**:
   ```
   Connection Name: My Server
   Host:            your-server.com
   Port:            22
   Username:        your-username
   Auth Method:     ○ Password  (selected)
   Password:        ••••••••
   ```

4. **Tap "Test Connection"**
   - Shows "Testing connection..."
   - If successful: "Connection successful!" ✅
   - If failed: Error message with details

5. **Tap "Save"**
   - Returns to main screen
   - Session is saved (in memory)

## Testing Without Real Server

If you don't have an SSH server handy, you can:

### Option 1: Use Docker (Linux/Mac)

```bash
# Start SSH server in Docker
docker run -d -p 2222:22 \
  -e SSH_USER=testuser \
  -e SSH_PASSWORD=testpass \
  lscr.io/linuxserver/openssh-server:latest

# In Zelland app, use:
# Host: localhost (or your machine IP from Android)
# Port: 2222
# Username: testuser
# Password: testpass
```

### Option 2: Use Public Test Server

**WARNING**: Only for testing! Do not use with sensitive data.

There are public SSH honeypots available (search "public SSH test servers"), but be careful as they are monitored and logged.

### Option 3: Skip SSH Testing for Now

You can proceed to Milestone 2 and test SSH + Zellij integration together.

## Project Structure Overview

```
zelland/
├── app/src/main/java/com/zelland/
│   ├── MainActivity.kt              # Main activity
│   ├── model/
│   │   ├── SSHConfig.kt            # SSH configuration
│   │   └── TerminalSession.kt      # Session model
│   ├── ssh/
│   │   └── SSHConnectionManager.kt # SSH connection handling
│   ├── ui/ssh/
│   │   └── SSHConfigActivity.kt    # SSH config screen
│   └── viewmodel/
│       └── TerminalViewModel.kt    # Session state management
├── app/src/main/res/
│   ├── layout/
│   │   ├── activity_main.xml       # Main layout
│   │   └── activity_ssh_config.xml # SSH config layout
│   └── values/
│       ├── strings.xml             # Text resources
│       ├── colors.xml              # Zelland colors
│       └── themes.xml              # Material 3 theme
└── docs/
    ├── DESIGN.md                   # Architecture
    ├── CHANGES_ZELLIJ.md          # Implementation roadmap
    └── SSH_INTEGRATION.md         # SSH/Zellij integration
```

## What Works Now (Milestone 1)

✅ Android project builds and runs
✅ SSH connection to remote hosts
✅ Password authentication
✅ Private key authentication
✅ Connection testing
✅ Basic session management
✅ Material 3 UI with Zelland branding

## What's Coming Next

⏳ **Milestone 2**: Start/stop Zellij web server remotely
⏳ **Milestone 3**: SSH port forwarding
⏳ **Milestone 4**: Load Zellij web in WebView
⏳ **Milestone 5**: Gesture controls for tab navigation

See [CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md) for complete roadmap.

## Common Issues

### Build Fails: "SSHJ not found"

**Solution**: Gradle sync may have failed. Try:
```bash
./gradlew clean
./gradlew build --refresh-dependencies
```

### App Crashes on Launch

**Solution**: Check logcat for errors:
```bash
adb logcat -s AndroidRuntime:E
```

Common causes:
- Missing dependencies (check build.gradle.kts)
- API level incompatibility (need API 24+)

### "Connection failed" Even With Correct Credentials

**Possible causes**:
1. **Network**: Android emulator uses `10.0.2.2` for host machine localhost
2. **Firewall**: SSH port (22) may be blocked
3. **Server**: SSH server not running or not accepting connections
4. **Auth**: Wrong username/password/key

**Debug**:
```bash
# Test from command line first
ssh username@your-server.com

# Check Android network
adb shell ping your-server.com
```

### Permission Denied

**Solution**: Ensure INTERNET permission in AndroidManifest.xml:
```xml
<uses-permission android:name="android.permission.INTERNET" />
```

## Debugging Tips

### View Logs
```bash
# All app logs
adb logcat -s Zelland:D

# SSH-specific logs
adb logcat -s SSHConnectionManager:D

# Clear logs
adb logcat -c
```

### Breakpoint Debugging

In Android Studio:
1. Set breakpoint in `SSHConnectionManager.connect()`
2. Run in debug mode (🐛)
3. Tap "Test Connection" in app
4. Step through code

## Next Steps

1. **Read the docs**:
   - [DESIGN.md](DESIGN.md) - Architecture overview
   - [SSH_INTEGRATION.md](SSH_INTEGRATION.md) - SSH/Zellij details

2. **Continue implementing**:
   - [CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md) - Milestone 2

3. **Set up Zellij**:
   ```bash
   # On your SSH server:
   curl -L zellij.dev/install.sh | bash
   zellij --version  # Should be 0.43.0+
   ```

## Getting Help

- **Issues**: Check [MILESTONE_1_COMPLETE.md](MILESTONE_1_COMPLETE.md)
- **Build Problems**: See [BUILD.md](BUILD.md)
- **Architecture Questions**: See [DESIGN.md](DESIGN.md)

## Success! 🎉

If you can:
- ✅ Build the app
- ✅ Run it on device/emulator
- ✅ Open SSH config dialog
- ✅ Test an SSH connection successfully

Then **Milestone 1 is complete** and you're ready for Milestone 2!

---

**Happy coding!** 🚀

Questions? Open an issue or check the documentation in the `docs/` folder.
