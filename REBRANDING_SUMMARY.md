# Zelland Rebranding Summary

## Overview

The project has been renamed from "Android Terminal App" / "droid" to **Zelland** (Zellij + Android).

This document summarizes all the changes made to align the documentation with the new branding.

---

## Files Updated

### 1. DESIGN.md
**Changes:**
- Title changed to "Zelland - Design Document"
- Overview updated to describe Zelland as "Zellij + Android"
- Package name changed from `com.example.droid` to `com.zelland`
- Test directory paths updated to use `com.zelland`

**Key Updates:**
```kotlin
// Before
app/src/main/java/com/example/droid/

// After
app/src/main/java/com/zelland/
```

### 2. CHANGES.md
**Changes:**
- Renamed to "Implementation Plan - Zelland (Original xterm.js Version)"
- Added note that this plan is superseded by CHANGES_ZELLIJ.md
- Marked as reference document only

**Status:** Kept for historical reference, but no longer the primary implementation plan.

### 3. CHANGES_ZELLIJ.md
**Changes:**
- Title changed to "Implementation Plan - Zelland"
- Overview updated with Zelland name and tagline
- Added "What is Zelland?" description

**Key Addition:**
```
What is Zelland? A native Android app that connects to remote Zellij
terminal multiplexer sessions via SSH, providing gesture-based navigation
and multi-host session management.
```

### 4. SSH_INTEGRATION.md
**Changes:**
- Title changed to "Zelland - SSH Integration with Zellij Web"
- Overview updated with Zelland description
- Package names changed from `com.example.droid.ssh` to `com.zelland.ssh`

**Code Updates:**
```kotlin
// Before
package com.example.droid.ssh

// After
package com.zelland.ssh
```

### 5. README.md (NEW)
**Created:** Complete project README with:
- Zelland branding and logo area
- Feature highlights
- Architecture diagram
- Quick start guide
- Building instructions
- Project structure
- Technology stack
- Roadmap
- FAQ section
- License information

### 6. BRANDING.md (NEW)
**Created:** Comprehensive branding guide including:
- Name etymology and pronunciation
- Taglines
- Package naming conventions
- Color scheme suggestions
- Icon concepts
- Typography guidelines
- Voice and tone
- Marketing copy
- Social media handles
- Attribution requirements

### 7. REBRANDING_SUMMARY.md (NEW)
**Created:** This document - summary of all rebranding changes

---

## Package Name Migration

### Old Structure
```
com.example.droid/
├── MainActivity.kt
├── ui/
│   ├── terminal/
│   └── keyboard/
├── viewmodel/
├── model/
└── bridge/
```

### New Structure
```
com.zelland/
├── MainActivity.kt
├── ui/
│   └── terminal/
│       ├── TerminalFragment.kt
│       └── TerminalWebView.kt
├── ssh/
│   └── SSHConnectionManager.kt
├── viewmodel/
│   └── TerminalViewModel.kt
└── model/
    └── TerminalSession.kt
```

**Note:** The keyboard package (ModBar/AlphaGrid) is not in the Zellij-based architecture, as Zellij web provides its own interface.

---

## Name Changes Summary

| Aspect | Old | New |
|--------|-----|-----|
| **App Name** | Android Terminal App / droid | Zelland |
| **Package** | com.example.droid | com.zelland |
| **Directory** | /home/njr/code/droid | Should rename to /home/njr/code/zelland |
| **Primary Doc** | CHANGES.md | CHANGES_ZELLIJ.md |
| **Architecture** | Standalone xterm.js | SSH → Zellij Web |

---

## Action Items

### Completed ✅
- [x] Update DESIGN.md with Zelland branding
- [x] Update CHANGES.md (marked as reference)
- [x] Update CHANGES_ZELLIJ.md with Zelland name
- [x] Update SSH_INTEGRATION.md with Zelland branding
- [x] Create comprehensive README.md
- [x] Create BRANDING.md guide
- [x] Update all package names in documentation

### Pending (When Implementation Starts)
- [ ] Rename project directory from `droid` to `zelland`
- [ ] Update Git repository name (if applicable)
- [ ] Create Android project with package `com.zelland`
- [ ] Update AndroidManifest.xml with app name "Zelland"
- [ ] Design app icon following branding guidelines
- [ ] Create launcher icon assets
- [ ] Add branding to About screen

### Future (Milestone 8: Polish)
- [ ] Finalize color scheme
- [ ] Design official logo
- [ ] Create app icon in multiple sizes
- [ ] Add splash screen with Zelland branding
- [ ] Write complete Play Store listing
- [ ] Create promotional screenshots
- [ ] Add branding to website/landing page (if created)

---

## Quick Reference

### Official Name
**Zelland** (always capitalized)

### Pronunciation
/ˈzel.lænd/ ("ZEL-land")

### Tagline
*"Zellij terminal multiplexer, in your pocket"*

### Package Name
`com.zelland`

### Primary Documentation
1. **README.md** - Project overview and quick start
2. **DESIGN.md** - Architecture and technical design
3. **CHANGES_ZELLIJ.md** - Implementation roadmap
4. **SSH_INTEGRATION.md** - SSH and Zellij integration
5. **BRANDING.md** - Brand guidelines

---

## Git Commands for Directory Rename

When ready to rename the project directory:

```bash
cd /home/njr/code
mv droid zelland
cd zelland

# If this is a Git repository:
git remote set-url origin https://github.com/yourusername/zelland.git

# Update any local references
grep -r "/droid" . --include="*.md" --include="*.kt"
# Manually update any hardcoded paths found
```

---

## Notes

1. **Backward Compatibility**: Since this is a new project (not yet released), there are no backward compatibility concerns with the rename.

2. **Search and Replace**: If implementing, search for any remaining instances of:
   - `droid` (case-insensitive)
   - `com.example.droid`
   - "Android Terminal App"
   - "terminal app" (generic references)

3. **Case Sensitivity**: Always capitalize "Zelland" as a proper noun. Lowercase only in package names (`com.zelland`).

4. **Attribution**: Remember to credit Zellij prominently in the app and documentation, as we're building directly on their web interface.

---

## Questions?

See [BRANDING.md](BRANDING.md) for detailed guidelines on using the Zelland name and brand.

For implementation details, refer to [CHANGES_ZELLIJ.md](CHANGES_ZELLIJ.md).
