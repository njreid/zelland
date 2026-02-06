# App Store Distribution Guide

This guide details the procedures for deploying the Tauri application to various app stores and packaging systems, along with critical guidelines to ensure listing approval.

## 1. Google Play Store (Android)

To list on Google Play, you must provide an Android App Bundle (`.aab`) signed with an upload key.

### Detailed Instructions

1.  **Generate Keystore:** Run this to create your signing key (keep this file safe!).

    ```bash
    keytool -genkey -v -keystore my-release-key.keystore -alias my-key-alias -keyalg RSA -keysize 2048 -validity 10000
    ```

2.  **Configure Tauri:** Create `src-tauri/gen/android/keystore.properties`:

    ```ini
    storePassword=your_password
    keyPassword=your_password
    keyAlias=my-key-alias
    storeFile=../../my-release-key.keystore
    ```

3.  **Build the Bundle:**

    ```bash
    npm run tauri android build -- --bundle aab
    ```

4.  **Upload:** Go to the [Google Play Console](https://play.google.com/console/), create an app, and upload the `.aab` found in `src-tauri/gen/android/app/build/outputs/bundle/universalRelease/`.

## 2. Apple App Store (iOS / macOS)

Apple requires an **Apple Developer Program** membership ($99/year) and a Mac for the final upload.

### Detailed Instructions

1.  **Certificates & Profiles:** In your [Apple Developer Account](https://developer.apple.com/account/), create an **App ID**, a **Distribution Certificate**, and a **Provisioning Profile**.
2.  **Entitlements:** Create `src-tauri/Entitlements.plist` to enable the Sandbox (required for the store):

    ```xml
    <plist version="1.0">
    <dict>
        <key>com.apple.security.app-sandbox</key><true/>
        <key>com.apple.security.network.client</key><true/>
    </dict>
    </plist>
    ```

3.  **Build & Sign:**

    ```bash
    # For iOS
    npm run tauri ios build
    # For macOS App Store
    npm run tauri build -- --bundles app --config src-tauri/tauri.appstore.conf.json
    ```

4.  **Upload:** Use the **Transporter** app (available on the Mac App Store) to upload your `.pkg` or `.ipa` file to App Store Connect.

## 3. F-Droid (Android)

F-Droid is for FOSS (Free and Open Source Software). They build your app from source on their own servers.

### Detailed Instructions

1.  **Prerequisites:** Your code must be hosted on a public Git repo (GitHub/GitLab) with an OSI-compliant license.
2.  **Submission:**
    *   Fork the [fdroiddata repository](https://gitlab.com/fdroid/fdroiddata).
    *   Create a new metadata file `metadata/com.your.appid.yml`.
3.  **Metadata Script (Template):**

    ```yaml
    Categories: [Utility]
    License: MIT
    SourceCode: https://github.com/user/my-tauri-app
    Builds:
      - versionName: 1.0.0
        versionCode: 1
        commit: v1.0.0
        subdir: src-tauri
        gradle: [yes]
        prebuild: sed -i 's/api/api-v2/' build.gradle
    ```

4.  **Submit:** Open a Merge Request to the main `fdroiddata` repo.

## 4. Arch User Repository (AUR)

Arch users prefer building from source or using a PKGBUILD that wraps your binary.

### Detailed Instructions

1.  **Create PKGBUILD:** Create a directory for your app and add a `PKGBUILD` file.
2.  **Script Example:**

    ```bash
    # Maintainer: Your Name <email@example.com>
    pkgname=my-tauri-app-bin
    pkgver=1.0.0
    pkgrel=1
    pkgdesc="A cool Tauri app"
    arch=('x86_64')
    url="https://github.com/user/repo"
    license=('MIT')
    depends=('webkit2gtk' 'gtk3' 'libappindicator-gtk3')
    source=("https://github.com/user/repo/releases/download/v$pkgver/app_$pkgver_amd64.deb")
    sha256sums=('PASTE_SHA256_HERE')

    package() {
      tar -xvf data.tar.xz -C "${pkgdir}/"
    }
    ```

3.  **Publish:**
    *   Initialize a git repo in the AUR: `git clone ssh://aur@aur.archlinux.org/my-tauri-app-bin.git`.
    *   Run `makepkg --printsrcinfo > .SRCINFO`.
    *   Commit and push.

## 5. Ubuntu (Snapcraft / PPA)

While `.deb` files work, **Snap** is the easiest way to reach all Ubuntu users (and other distros).

### Detailed Instructions

1.  **Install Snapcraft:** `sudo snap install snapcraft --classic`.
2.  **Create `snapcraft.yaml`:**

    ```yaml
    name: my-tauri-app
    version: '1.0.0'
    summary: My Tauri App
    description: A longer description here.
    base: core22
    confinement: strict

    parts:
      my-app:
        plugin: dump
        source: src-tauri/target/release/bundle/deb/my-app_1.0.0_amd64.deb
        source-type: deb

    apps:
      my-tauri-app:
        command: usr/bin/my-tauri-app
        extensions: [gnome]
    ```

3.  **Build & Push:**

    ```bash
    snapcraft
    snapcraft login
    snapcraft upload --release=stable my-tauri-app_1.0.0_amd64.snap
    ```

---

# Listing Approval Guide: Principles & Libraries to Avoid

When listing a Tauri app, rejections rarely happen because of Tauri itself. Instead, they occur because of **how you use the bridge** between your web frontend and the native OS.

## 1. Apple App Store (iOS & macOS)

Apple is the most restrictive. They focus on **Privacy Manifests**, **Sandboxing**, and **Private APIs**.

### ❌ Avoid: "Private API" Calls

Avoid any Rust crates or JavaScript libraries that attempt to access internal macOS/iOS functions not documented by Apple.

*   **The Risk:** Apple uses automated scanners during the upload process. If they find symbols like `CAContext` or `NSNextStepFrame` (often found in older Electron-like setups), you will be instantly rejected.
*   **Tauri Tip:** Stick to official [Tauri Plugins](https://v2.tauri.app/plugin/). They are designed to use Apple-approved public APIs.

### ❌ Avoid: Libraries lacking "Privacy Manifests"

As of 2024, Apple requires a `PrivacyInfo.xcprivacy` file if you use certain "Required Reason" APIs (like disk access or system boot time).

*   **The Risk:** If a Rust crate you use (e.g., for file system stats) touches these APIs without a manifest, the build will be rejected.
*   **Solution:** Ensure your dependencies are up-to-date. Tauri v2 handles much of this, but you must manually declare data collection in your own manifest.

### ❌ Avoid: External Payment Links

*   **The Risk:** If you sell digital goods (subscriptions, features), you **must** use Apple's In-App Purchase (IAP).
*   **Principle:** Do not include libraries like `Stripe` or `Lemon Squeezy` in your frontend to bypass the 30% "Apple Tax" for digital content. Apple will reject your app if they find a "Buy" button that leads to an external website.

## 2. Google Play Store (Android)

Google is more lenient on code but very strict on **Permissions** and **Clear Disclosure**.

### ❌ Avoid: Unnecessary "Dangerous" Permissions

*   **The Risk:** Requesting `READ_EXTERNAL_STORAGE` or `ACCESS_FINE_LOCATION` when your app doesn't strictly need them for its core feature.
*   **Principle:** If your app is a "Note Taker" but asks for "Background Location," Google will likely reject it during the manual review phase.
*   **Tauri Tip:** Check your `AndroidManifest.xml` generated by Tauri and remove any permissions you aren't actively using.

### ❌ Avoid: Background Execution Libraries

Avoid libraries that keep the app running in the background indefinitely without a "Foreground Service" notification. Google will flag this as battery drain/malware behavior.

## 3. F-Droid (Android)

F-Droid has a "zero-proprietary" policy. If your code isn't 100% Open Source, it won't get in.

### ❌ Avoid: Proprietary SDKs

*   **Google Play Services:** Do not use the official Google Maps or Firebase SDKs. F-Droid cannot build these because they are closed-source.
*   **Alternative:** Use [MicroG](https://microg.org/) compatible libraries or FOSS alternatives like **MapLibre** instead of Google Maps.

### ❌ Avoid: Binary Blobs

*   **The Risk:** Some Rust crates download a pre-compiled `.so` or `.a` library during the build process to save time.
*   **Principle:** F-Droid builds everything from source. If a crate requires a pre-built binary, the build will fail on their "Buildserver," and your app will be rejected.

## 4. General Principles for All Stores

### The "Website in a Box" Trap

Both Apple and Google will reject apps that are "merely a website."

*   **Principle:** Your Tauri app must provide **functional value** offline or use native features (notifications, local storage, file system).
*   **Design:** Avoid using a standard `header/footer` that looks like a website. Use mobile-first design patterns (bottom tabs, native-feeling transitions).

### Data Privacy (The "Clear Label" Rule)

*   **Principle:** If you use a library for analytics (like `PostHog` or `Sentry`), you **must** disclose this in the Store's Privacy Section.
*   **Library Choice:** Prefer local-first or privacy-focused libraries (like `Plausible`) to make your "Privacy Label" look better to users.

### Summary Table: What to Check

| Platform | Avoid at all costs | Preferred Alternative |
| :--- | :--- | :--- |
| **Apple** | External Payment SDKs (Stripe, etc.) | Apple In-App Purchase |
| **Google** | Excessive Permissions (`MANAGE_EXTERNAL_STORAGE`) | Scoped Storage / File Picker |
| **F-Droid** | Firebase / Google Play Services | Appwrite / Supabase (Self-hosted) |
| **All** | Tracking without Consent | Privacy-first analytics / Opt-in prompts |
