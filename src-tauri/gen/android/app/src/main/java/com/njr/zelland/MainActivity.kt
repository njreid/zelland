package com.njr.zelland

import android.os.Bundle
import android.util.Base64
import javax.crypto.Cipher

class MainActivity : TauriActivity() {
    private lateinit var keyStoreManager: KeyStoreManager
    private lateinit var biometricManager: BiometricManager

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        keyStoreManager = KeyStoreManager()
        biometricManager = BiometricManager(this)
    }

    // JNI Methods
    fun generateBiometricKey(alias: String): Boolean {
        return try {
            keyStoreManager.generateBiometricKey(alias)
            true
        } catch (e: Exception) {
            false
        }
    }

    fun hasBiometricKey(alias: String): Boolean {
        return try {
            keyStoreManager.hasKey(alias)
        } catch (e: Exception) {
            false
        }
    }

    interface JniCallback {
        fun onComplete(success: Boolean, error: String?)
    }

    interface JniDecryptCallback {
        fun onComplete(success: Boolean, data: String?, error: String?)
    }

    fun authenticate(alias: String, title: String, subtitle: String, callback: JniCallback) {
        try {
            val cipher = keyStoreManager.getCipher(alias, Cipher.DECRYPT_MODE)
            biometricManager.authenticate(title, subtitle, cipher, object : BiometricManager.AuthCallback {
                override fun onSuccess(cipher: Cipher?) {
                    callback.onComplete(true, null)
                }

                override fun onError(err: String) {
                    callback.onComplete(false, err)
                }
            })
        } catch (e: Exception) {
            callback.onComplete(false, e.message)
        }
    }

    fun authenticateAndDecrypt(
        alias: String,
        title: String,
        subtitle: String,
        encryptedDataBase64: String,
        callback: JniDecryptCallback
    ) {
        try {
            val cipher = keyStoreManager.getCipher(alias, Cipher.DECRYPT_MODE)
            biometricManager.authenticate(title, subtitle, cipher, object : BiometricManager.AuthCallback {
                override fun onSuccess(authedCipher: Cipher?) {
                    try {
                        val encryptedData = Base64.decode(encryptedDataBase64, Base64.NO_WRAP)
                        val decrypted = keyStoreManager.decryptData(authedCipher!!, encryptedData)
                        val decryptedStr = String(decrypted, Charsets.UTF_8)
                        callback.onComplete(true, decryptedStr, null)
                    } catch (e: Exception) {
                        callback.onComplete(false, null, "Decryption failed: ${e.message}")
                    }
                }

                override fun onError(err: String) {
                    callback.onComplete(false, null, err)
                }
            })
        } catch (e: Exception) {
            callback.onComplete(false, null, e.message)
        }
    }

    fun encryptWithBiometricKey(alias: String, data: String): String? {
        return try {
            val (iv, encrypted) = keyStoreManager.encryptData(alias, data.toByteArray(Charsets.UTF_8))
            val combined = iv + encrypted
            Base64.encodeToString(combined, Base64.NO_WRAP)
        } catch (e: Exception) {
            null
        }
    }
}
