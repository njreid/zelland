# Zelland - Branding Guide

## Name

**Zelland** = Zellij + Android

A portmanteau that combines:
- **Zellij**: The terminal multiplexer we integrate with
- **Android**: The mobile platform

Pronunciation: /ˈzel.lænd/ ("ZEL-land")

## Tagline

*"Zellij terminal multiplexer, in your pocket"*

Alternative taglines:
- *"Terminal multiplexing on the go"*
- *"Your Zellij sessions, anywhere"*
- *"Mobile terminal, multiplexed"*

## Package Name

**Primary**: `com.zelland`

**Full paths**:
- Main package: `com.zelland`
- SSH integration: `com.zelland.ssh`
- UI components: `com.zelland.ui`
- ViewModels: `com.zelland.viewmodel`
- Models: `com.zelland.model`

## App Metadata

### Application Name
- **Short name**: Zelland
- **Long name**: Zelland - Zellij for Android
- **Description**: Connect to remote Zellij terminal sessions from your Android device

### Keywords
- zellij
- terminal
- ssh
- multiplexer
- tmux alternative
- remote terminal
- developer tools
- command line
- shell

## Brand Colors

Suggested color scheme (to be finalized during M8: Polish):

### Primary
- **Zellij Orange**: `#FF6B35` (from Zellij branding)
- **Android Green**: `#3DDC84` (Material Design)
- **Zelland Primary**: Blend/gradient of both

### Accent
- **SSH Blue**: `#2196F3` (for connection indicators)
- **Success Green**: `#4CAF50` (connected state)
- **Warning Yellow**: `#FFC107` (reconnecting/idle)
- **Error Red**: `#F44336` (disconnected/error)

### Terminal
- **Background**: `#1E1E1E` (dark mode default)
- **Foreground**: `#D4D4D4`
- **Selection**: `#264F78`

## Icon Concept

The app icon should combine:
1. Terminal/command-line aesthetic (monospace font, cursor)
2. Zellij's tiling/multiplexing concept (grid, panes)
3. Android's Material Design language (rounded, clean)
4. Connectivity concept (network, SSH)

Possible designs:
- **Option A**: Grid of terminal panes with Android accent
- **Option B**: Stylized "Z" in monospace font with connection lines
- **Option C**: Terminal window with Zellij's signature orange border

## Typography

### App UI
- **Primary**: Roboto (Android system font)
- **Monospace (code/logs)**: JetBrains Mono or Source Code Pro

### Terminal Display
- **Font**: Determined by Zellij web (typically a monospace web font)
- **Size**: User-configurable in settings

## Voice & Tone

### Documentation
- **Technical but approachable**: Clear explanations for developers
- **Assume familiarity** with terminal concepts, SSH, Zellij
- **Step-by-step** for setup and configuration

### UI Copy
- **Concise**: Mobile screens are small
- **Action-oriented**: "Connect", "Disconnect", "Reconnect"
- **Status-focused**: "Connected to server", "Starting Zellij..."

### Error Messages
- **Helpful**: Explain what went wrong
- **Actionable**: Suggest how to fix it
- **Friendly**: Avoid blame ("Connection failed" not "You failed to connect")

Examples:
- ✅ "SSH connection failed. Check your credentials and try again."
- ✅ "Zellij not found on remote host. Install Zellij 0.43.0+ to continue."
- ❌ "Error: Connection refused" (too technical, not actionable)

## Marketing Copy

### One-sentence pitch
*"Zelland brings Zellij's powerful terminal multiplexer to your Android device via SSH."*

### Short description (80 chars)
*"Access remote Zellij sessions from Android with gesture controls and multi-host support"*

### Long description (Google Play Store)
```
Zelland connects you to your remote Zellij terminal sessions from anywhere.

Whether you're managing servers, developing software, or working on the go, Zelland provides a seamless mobile interface for Zellij's powerful terminal multiplexing features.

KEY FEATURES:
• SSH Integration - Secure connections with password or key authentication
• Zellij Web - Automatically manages remote Zellij web servers
• Gesture Controls - Swipe to navigate tabs and switch between hosts
• Multi-Session - Connect to multiple servers simultaneously
• Session Persistence - Sessions continue running when you disconnect
• Native Android - Optimized for mobile with Material Design

PERFECT FOR:
• DevOps engineers managing remote infrastructure
• Developers working on cloud servers
• System administrators
• Anyone who loves terminal multiplexers

REQUIREMENTS:
• Android 7.0 or higher
• SSH access to a remote server
• Zellij 0.43.0+ installed on remote server

Open source and community-driven. Join us on GitHub!
```

## File Naming Conventions

### Documentation
- Use underscores for multi-word files: `SSH_INTEGRATION.md`
- ALL_CAPS for major docs: `README.md`, `DESIGN.md`
- Descriptive names: `CHANGES_ZELLIJ.md` (not just `CHANGES.md`)

### Code Files
- PascalCase for classes: `TerminalFragment.kt`
- camelCase for files with multiple words: `SSHConnectionManager.kt`
- Package structure mirrors feature: `ssh/`, `ui/terminal/`

### Resources
- lowercase with underscores: `activity_main.xml`
- Prefix by type: `ic_launcher.png`, `btn_connect.xml`

## Social Media

### Hashtags
- Primary: `#zelland`
- Related: `#zellij` `#terminal` `#android` `#ssh` `#tmux` `#commandline`

### Handles (suggested)
- GitHub: `zelland` or `zelland-app`
- Twitter/X: `@zelland_app`
- Reddit: `r/zelland`

## Attribution

Always credit the projects we build upon:
- **Zellij** - The terminal multiplexer
- **SSHJ** - SSH library
- **Android Open Source Project** - Platform

Include in About screen:
```
Zelland is powered by:
• Zellij (MIT License) - zellij.dev
• SSHJ (Apache 2.0) - github.com/hierynomus/sshj
• Android Open Source Project (Apache 2.0)
```

## Logo Guidelines (To Be Designed)

When the logo is created, document:
- Minimum size (for app icon, splash screen)
- Clear space around logo
- Color variations (full color, monochrome, dark mode)
- Acceptable uses and restrictions
- Logo file formats and exports

## Directory Structure Migration

**Note**: This project currently lives in `/home/njr/code/droid/`.

To align with the new branding, consider renaming:
```bash
mv /home/njr/code/droid /home/njr/code/zelland
```

Then update any hardcoded paths in:
- Git remote URLs (if applicable)
- Build scripts
- README examples
- Documentation references

## Changelog

- **2026-01-18**: Initial branding document created
  - App renamed from "droid" / "Android Terminal App" to "Zelland"
  - Package name changed to `com.zelland`
  - All documentation updated with new branding
