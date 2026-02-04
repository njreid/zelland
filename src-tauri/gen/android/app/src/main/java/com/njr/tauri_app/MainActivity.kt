package com.njr.tauri_app

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import app.tauri.plugin.JSObject

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
    if (intent?.action == Intent.ACTION_SEND && intent.type == "text/plain") {
      val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)
      if (sharedText != null) {
        val payload = JSObject()
        payload.put("text", sharedText)
        this.app.emit("intent-received", payload)
      }
    }
  }
}