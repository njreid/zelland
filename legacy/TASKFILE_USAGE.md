# Taskfile Usage Guide

Zelland uses [Task](https://taskfile.dev) for build automation and development workflows.

## Installation

### Install Task

```bash
# macOS
brew install go-task

# Linux
sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b ~/.local/bin

# Or via Go
go install github.com/go-task/task/v3/cmd/task@latest
```

### Verify Installation

```bash
task --version
```

## Quick Start

```bash
# List all available tasks
task

# Build debug APK
task build

# Install on connected device
task install

# Build, install, and run
task run
```

## Common Tasks

### Local Development

```bash
# Full development workflow (build → install → run → logs)
task dev

# Clean build
task clean

# Build debug APK
task build

# Build release APK
task build-release

# Install on local device
task install

# Run app (build + install + launch)
task run
```

### Remote Development

```bash
# Push to remote emulator and show logs
task dev-remote -- myserver.example.com

# Just push APK to remote emulator
task push -- myserver.example.com

# Push release APK to remote
task push-release -- hostname

# View logs from remote device
task logs -- hostname
```

### Testing & Quality

```bash
# Run unit tests
task test

# Run lint checks
task lint

# Run all checks (test + lint)
task check

# Generate and open test report
task test-report

# Generate and open lint report
task lint-report
```

### Device Management

```bash
# List connected devices
task devices

# Connect to remote ADB server
task connect -- hostname

# Disconnect from remote ADB
task disconnect -- hostname

# Disconnect from all remotes
task disconnect-all

# Open ADB shell
task shell

# Remote shell
task shell -- hostname
```

### Debugging & Logs

```bash
# View app logs (local device)
task logs

# View logs from remote device
task logs -- hostname

# View all system logs
task logs-all

# Clear logs
task logs-clear

# Save logs to file
task logcat-save
```

### Utilities

```bash
# Take screenshot
task screenshot

# Take screenshot from remote
task screenshot -- hostname

# Uninstall app
task uninstall

# Uninstall from remote
task uninstall-remote -- hostname

# Show build information
task info

# Setup development environment
task setup
```

## Remote Emulator Usage

### Prerequisites

On the remote host, ensure ADB server is listening on 0.0.0.0:

```bash
# On remote host
adb kill-server
adb -a nodaemon server start
```

Or use SSH port forwarding:

```bash
# On local machine
ssh -L 5555:localhost:5555 user@remote-host
```

### Connecting to Remote Emulator

```bash
# Connect to remote
task connect -- myserver.example.com

# Verify connection
task devices

# Push and run app
task push -- myserver.example.com

# View logs
task logs -- myserver.example.com
```

### Full Remote Workflow Example

```bash
# 1. Connect to remote emulator
task connect -- 192.168.1.100

# 2. Push app and launch
task push -- 192.168.1.100

# 3. Watch logs
task logs -- 192.168.1.100

# 4. When done, disconnect
task disconnect -- 192.168.1.100
```

### Using with Tailscale

If your remote host is on Tailscale:

```bash
# Connect using Tailscale hostname
task connect -- myserver.tailnet-abc.ts.net

# Or using Tailscale IP
task connect -- 100.64.1.5

# Push app
task push -- myserver.tailnet-abc.ts.net
```

## Task Aliases

Some tasks have short aliases for convenience:

```bash
task b       # build
task br      # build-release
task i       # install
task r       # run
task t       # test
task l       # lint
task p       # push
task d       # devices
```

## Examples

### Example 1: Quick Local Test

```bash
task clean && task run
```

### Example 2: Release Build

```bash
# Full release workflow
task release

# This runs:
# - clean
# - lint
# - test
# - build-release
```

### Example 3: Push to Multiple Remote Devices

```bash
# Push to server 1
task push -- server1.example.com

# Push to server 2
task push -- server2.example.com

# View devices
task devices
```

### Example 4: Debug Remote Issue

```bash
# Connect and view logs in real-time
task connect -- myserver
task logs -- myserver

# Clear old logs first
task logs-clear -- myserver
task push -- myserver
task logs -- myserver
```

### Example 5: Take Screenshots

```bash
# Local screenshot
task screenshot

# Remote screenshot
task screenshot -- remote-host
```

## Customization

### Modify Variables

Edit `Taskfile.yml` to change defaults:

```yaml
vars:
  APP_PACKAGE: com.zelland          # Change package name
  APP_ACTIVITY: com.zelland.MainActivity  # Change main activity
  APK_DEBUG: app/build/outputs/apk/debug/app-debug.apk
  APK_RELEASE: app/build/outputs/apk/release/app-release.apk
```

### Add Custom Tasks

Add your own tasks to `Taskfile.yml`:

```yaml
tasks:
  my-task:
    desc: My custom task
    cmds:
      - echo "Running my task"
      - ./gradlew myCustomGradleTask
```

## Troubleshooting

### Task not found

```bash
# Ensure task is installed
task --version

# If not found, install it (see Installation above)
```

### Cannot connect to remote emulator

```bash
# Check if remote ADB server is running
ssh user@remote-host "adb devices"

# Start ADB server on remote (listening on all interfaces)
ssh user@remote-host "adb -a nodaemon server start"

# Or use SSH tunnel
ssh -L 5555:localhost:5555 user@remote-host
task push -- localhost
```

### APK not found

```bash
# Build first
task build

# Check APK location
ls -lh app/build/outputs/apk/debug/app-debug.apk
```

### Device unauthorized

```bash
# Accept authorization on device
# Then try again:
task devices
```

### Multiple devices connected

```bash
# List devices
task devices

# Specify device
adb -s <device-id> install app/build/outputs/apk/debug/app-debug.apk
```

## Integration with IDEs

### VS Code

Add to `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Build and Run",
      "type": "shell",
      "command": "task run",
      "group": "build"
    }
  ]
}
```

### IntelliJ IDEA / Android Studio

1. Go to Settings → Tools → External Tools
2. Add new tool:
   - Name: `Task Build`
   - Program: `task`
   - Arguments: `build`
   - Working directory: `$ProjectFileDir$`

## CI/CD Integration

### GitHub Actions

```yaml
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Task
        run: |
          sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

      - name: Build and Test
        run: |
          task clean
          task check
          task build
```

### GitLab CI

```yaml
build:
  stage: build
  before_script:
    - curl -sL https://taskfile.dev/install.sh | sh -s -- -d -b /usr/local/bin
  script:
    - task clean
    - task check
    - task build
```

## Further Reading

- [Task Documentation](https://taskfile.dev/)
- [Zelland BUILD.md](BUILD.md) - Build system details
- [Zelland QUICKSTART.md](QUICKSTART.md) - Quick setup guide

## Summary of Key Commands

| Command | Description |
|---------|-------------|
| `task` | List all tasks |
| `task build` | Build debug APK |
| `task run` | Build, install, and run |
| `task push -- hostname` | Push to remote emulator |
| `task dev` | Local dev workflow (build + run + logs) |
| `task dev-remote -- hostname` | Remote dev workflow |
| `task logs` | View app logs |
| `task test` | Run tests |
| `task lint` | Run lint checks |
| `task release` | Create release build |
| `task devices` | List connected devices |
| `task clean` | Clean build |

For a complete list, run `task` in the project directory.
