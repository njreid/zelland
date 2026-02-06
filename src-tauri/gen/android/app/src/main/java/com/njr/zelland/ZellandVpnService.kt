package com.njr.zelland

import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.content.Intent
import android.util.Log

class ZellandVpnService : VpnService() {
    private var tunnel: ParcelFileDescriptor? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        if (action == "STOP") {
            stopSelf()
            return START_NOT_STICKY
        }

        // Configuration from intent
        val address = intent?.getStringExtra("address") ?: "10.0.0.2"
        val mtu = intent?.getIntExtra("mtu", 1280) ?: 1280

        try {
            val builder = Builder()
                .setSession("Zelland")
                .addAddress(address, 24)
                .setMtu(mtu)
                // Add routes if needed
                // .addRoute("10.0.0.0", 24)
            
            tunnel = builder.establish()
            Log.i("ZellandVPN", "Tunnel established: ${tunnel?.fd}")
            
            // Pass the FD to Rust if needed, but for now we'll just keep it alive
            // In a real implementation, we'd use a JNI call to send tunnel.fd to Rust
            
        } catch (e: Exception) {
            Log.e("ZellandVPN", "Failed to establish tunnel", e)
        }

        return START_STICKY
    }

    override fun onDestroy() {
        tunnel?.close()
        super.onDestroy()
    }
}
