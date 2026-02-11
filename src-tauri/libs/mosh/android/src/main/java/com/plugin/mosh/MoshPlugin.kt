package com.plugin.mosh

import android.app.Activity
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

@TauriPlugin
class MoshPlugin(private val activity: Activity) : Plugin(activity) {
    // No commands needed, we just bundle libs
}