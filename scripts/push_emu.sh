#!/bin/bash
set -e

SDK_PATH="$HOME/Android/Sdk"
ADB="$SDK_PATH/platform-tools/adb"
EMULATOR_BIN="$SDK_PATH/emulator/emulator"

# Improved check: look for any device matching "emulator-"
if ! "$ADB" devices | grep "emulator-" | grep -v "offline" > /dev/null; then
  
  # Find a Pixel 9 AVD or fallback to the first available
  AVD=$("$EMULATOR_BIN" -list-avds | grep -i "Pixel_9" | head -n 1)
  if [ -z "$AVD" ]; then
    AVD=$("$EMULATOR_BIN" -list-avds | head -n 1)
  fi
  
  if [ -z "$AVD" ]; then
    echo "❌ No Android Virtual Devices (AVDs) found."
    echo "Please create one in Android Studio."
    exit 1
  fi
  
  echo "🚀 Starting emulator: $AVD"
  "$EMULATOR_BIN" -avd "$AVD" -feature -Vulkan -netdelay none -netspeed full > /dev/null 2>&1 &
  
  echo "⏳ Waiting for device to attach..."
  "$ADB" wait-for-device
  
  # Wait for boot completion
  echo "⏳ Waiting for boot to complete..."
  while [ "$("$ADB" shell getprop sys.boot_completed | tr -d '\r')" != "1" ]; do sleep 1; done
  echo "✓ Emulator ready"
else
  echo "✓ Emulator already running"
fi
