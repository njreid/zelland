package com.njr.zelland

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.util.*
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey

class KeyStoreManager {
    private val KEYSTORE_NAME = "AndroidKeyStore"

    fun generateBiometricKey(alias: String) {
        val keyGenerator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_NAME
        )
        val spec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setUserAuthenticationRequired(true)
            .setUserAuthenticationParameters(
                0, // timeout
                KeyProperties.AUTH_BIOMETRIC_STRONG
            )
            .build()
        keyGenerator.init(spec)
        keyGenerator.generateKey()
    }

    fun getCipher(alias: String, mode: Int): Cipher {
        val keyStore = KeyStore.getInstance(KEYSTORE_NAME)
        keyStore.load(null)
        val key = keyStore.getKey(alias, null) as SecretKey
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(mode, key)
        return cipher
    }

    fun encryptData(alias: String, data: ByteArray): Pair<ByteArray, ByteArray> {
        val keyStore = KeyStore.getInstance(KEYSTORE_NAME)
        keyStore.load(null)
        val key = keyStore.getKey(alias, null) as SecretKey
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val encrypted = cipher.doFinal(data)
        return Pair(cipher.iv, encrypted)
    }

    fun decryptData(cipher: Cipher, data: ByteArray): ByteArray {
        return cipher.doFinal(data)
    }

    fun hasKey(alias: String): Boolean {
        val keyStore = KeyStore.getInstance(KEYSTORE_NAME)
        keyStore.load(null)
        return keyStore.containsAlias(alias)
    }
}
