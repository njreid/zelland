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

        val address = intent?.getStringExtra("address") ?: "10.0.0.2"
        val mtu = intent?.getIntExtra("mtu", 1280) ?: 1280

        try {
            val builder = Builder()
                .setSession("zelland")
                .addAddress(address, 24)
                .setMtu(mtu)
            
            tunnel = builder.establish()
            Log.i("zellandVPN", "Tunnel established: ${tunnel?.fd}")
            
        } catch (e: Exception) {
            Log.e("zellandVPN", "Failed to establish tunnel", e)
        }

        return START_STICKY
    }

    override fun onDestroy() {
        tunnel?.close()
        super.onDestroy()
    }
}
