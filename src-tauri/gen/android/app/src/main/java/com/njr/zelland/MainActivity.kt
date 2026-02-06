package com.njr.zelland

import android.os.Bundle
import android.content.Intent
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    handleIntent(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    handleIntent(intent)
  }

  private fun handleIntent(intent: Intent?) {
    if (intent?.action == Intent.ACTION_SEND) {
      val text = intent.getStringExtra(Intent.EXTRA_TEXT)
      if (text != null) {
        // Emit event to Tauri
        triggerEmit("intent://received", text)
      }
    }
  }

  private fun triggerEmit(event: String, payload: String) {
    // This is a placeholder for actual Tauri emission logic from Kotlin
    // Usually handled by a plugin or direct webview access
  }
}
