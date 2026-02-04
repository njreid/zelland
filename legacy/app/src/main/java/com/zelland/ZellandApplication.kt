package com.zelland

import android.app.Application
import org.bouncycastle.jce.provider.BouncyCastleProvider
import java.security.Security

class ZellandApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        setupBouncyCastle()
    }

    private fun setupBouncyCastle() {
        val provider = Security.getProvider(BouncyCastleProvider.PROVIDER_NAME)
        if (provider == null) {
            // Bouncy Castle is not registered, so we register it
            Security.addProvider(BouncyCastleProvider())
        } else {
            // Bouncy Castle is already registered.
            // If it's the Android version (which is stripped down), we might want to remove it and add the full version.
            // However, removing the system provider can be risky.
            // A safer approach is to insert our provider at the beginning of the list.
            if (provider.javaClass.name.equals("com.android.org.bouncycastle.jce.provider.BouncyCastleProvider")) {
                Security.removeProvider(BouncyCastleProvider.PROVIDER_NAME)
                Security.insertProviderAt(BouncyCastleProvider(), 1)
            }
        }
    }
}
