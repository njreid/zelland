# Building Zelland

## Prerequisites

- **Android Studio**: Hedgehog (2023.1.1) or later
- **JDK**: Java 17 or later
- **Android SDK**: API 36 (compile), API 24+ (minimum)
- **Gradle**: 8.2.0 (included via wrapper)

## Quick Start

### 1. Clone or Open Project

```bash
cd /home/njr/code/droid
# Or rename to:
# mv /home/njr/code/droid /home/njr/code/zelland
# cd /home/njr/code/zelland
```

### 2. Open in Android Studio

```
File → Open → Select project directory
```

Wait for Gradle sync to complete.

### 3. Build

**Option A: Using Task (Recommended)**
```bash
# Install Task first (https://taskfile.dev)
# macOS: brew install go-task
# Linux: sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b ~/.local/bin

# Build debug APK
task build

# Build and run
task run

# Full development workflow (build + install + run + logs)
task dev

# See all available tasks
task
```

**Option B: Using Gradle directly**
```bash
# Via command line
./gradlew assembleDebug

# Or in Android Studio
Build → Make Project (Ctrl+F9)
```

### 4. Run

**Option A: Using Task**
```bash
# Build, install, and launch
task run

# Or with logs
task dev
```

**Option B: Android Studio**
1. Connect device or start emulator
2. Run → Run 'app' (Shift+F10)

**Option C: Gradle**
```bash
# Install debug APK
./gradlew installDebug

# Or build and install
./gradlew assembleDebug && adb install app/build/outputs/apk/debug/app-debug.apk
```

## Task Automation

Zelland includes a `Taskfile.yml` for streamlined development workflows.

### Quick Reference

```bash
# Local development
task build              # Build debug APK
task install            # Install on device
task run                # Build, install, and launch
task dev                # Full workflow with logs

# Remote development (push to emulator on another host)
task push -- hostname   # Build and push to remote emulator
task dev-remote -- host # Full remote workflow

# Testing & quality
task test               # Run unit tests
task lint               # Run lint checks
task check              # Run all checks

# Device management
task devices            # List connected devices
task logs               # View app logs
task logs -- hostname   # View logs from remote device

# Utilities
task clean              # Clean build
task release            # Build release APK
```

### Remote Emulator Example

Push and run on a remote emulator:

```bash
# Connect to remote emulator at myserver.example.com:5555
task push -- myserver.example.com

# View logs from remote device
task logs -- myserver.example.com

# Full workflow (clean + build + push + logs)
task dev-remote -- myserver.example.com
```

**See [TASKFILE_USAGE.md](TASKFILE_USAGE.md) for complete documentation.**

## Testing

### Unit Tests (JVM)
```bash
./gradlew test
```

### Instrumentation Tests (Device/Emulator)
```bash
./gradlew connectedAndroidTest
```

**Note**: Instrumentation tests require:
- Device/emulator running API 24+
- Test SSH server accessible from device
- Network connectivity

## Project Structure

```
.
├── app/
│   ├── build.gradle.kts          # App-level Gradle config
│   ├── proguard-rules.pro        # ProGuard/R8 rules
│   └── src/
│       ├── main/
│       │   ├── java/com/zelland/ # Kotlin source files
│       │   ├── res/              # Resources (layouts, strings, etc.)
│       │   └── AndroidManifest.xml
│       ├── test/                 # Unit tests
│       └── androidTest/          # Instrumentation tests
├── build.gradle.kts              # Project-level Gradle config
├── settings.gradle.kts           # Gradle settings
└── gradle.properties             # Gradle properties

```

## Key Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| SSHJ | 0.38.0 | SSH client library |
| BouncyCastle | 1.78 | Cryptography (required by SSHJ) |
| SLF4J Android | 1.7.36 | Logging for SSHJ |
| Material Components | 1.11.0 | UI components |
| ViewPager2 | 1.0.0 | Session swiping |
| Coroutines | 1.7.3 | Async operations |

See `app/build.gradle.kts` for complete dependency list.

## Common Build Issues

### Issue: "SSHJ classes not found"

**Solution**: Ensure BouncyCastle dependencies are included:
```kotlin
implementation("org.bouncycastle:bcprov-jdk18on:1.78")
implementation("org.bouncycastle:bcpkix-jdk18on:1.78")
```

### Issue: "Duplicate class" errors

**Solution**: Check `packaging` section in build.gradle.kts excludes duplicate META-INF files.

### Issue: "Failed to resolve: com.hierynomus:sshj"

**Solution**: Ensure mavenCentral() is in repositories:
```kotlin
repositories {
    google()
    mavenCentral()
}
```

### Issue: SSL/TLS errors during Gradle sync

**Solution**:
1. Update Gradle wrapper: `./gradlew wrapper --gradle-version=8.2`
2. Check network/proxy settings
3. Try: `./gradlew --refresh-dependencies`

## Release Build

### 1. Configure Signing

Create `keystore.properties` (DO NOT commit):
```properties
storePassword=YOUR_KEYSTORE_PASSWORD
keyPassword=YOUR_KEY_PASSWORD
keyAlias=zelland
storeFile=/path/to/keystore.jks
```

### 2. Build Release APK

```bash
./gradlew assembleRelease
```

Output: `app/build/outputs/apk/release/app-release.apk`

### 3. Build App Bundle (for Play Store)

```bash
./gradlew bundleRelease
```

Output: `app/build/outputs/bundle/release/app-release.aab`

## Debugging

### View Logs
```bash
# All logs
adb logcat

# Filter by tag
adb logcat -s Zelland:D

# Clear logs
adb logcat -c
```

### Debug Build
The debug build includes:
- Source maps for debugging
- Verbose logging enabled
- No code obfuscation
- No minification

### Profile Performance
```bash
# Generate build scan
./gradlew assembleDebug --scan
```

## Clean Build

If experiencing build issues:
```bash
# Clean build directories
./gradlew clean

# Invalidate caches (Android Studio)
File → Invalidate Caches → Invalidate and Restart
```

## Next Steps

After successful build:
1. See [CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md) for implementation roadmap
2. Continue with Milestone 2: Zellij Web Server Management
3. Set up a test SSH server with Zellij installed

## Support

- **Issues**: Open issue on GitHub
- **Documentation**: See [README.md](README.md), [DESIGN.md](DESIGN.md)
- **Architecture**: See [SSH_INTEGRATION.md](SSH_INTEGRATION.md)
