package com.njr.zelland

import android.os.Bundle
import android.util.Base64
import android.util.Log
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.core.view.GestureDetectorCompat
import javax.crypto.Cipher

class MainActivity : TauriActivity() {
    private lateinit var keyStoreManager: KeyStoreManager
    private lateinit var biometricManager: BiometricManager
    private var surfaceView: SurfaceView? = null
    private lateinit var mDetector: GestureDetectorCompat

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        keyStoreManager = KeyStoreManager()
        biometricManager = BiometricManager(this)
        
        mDetector = GestureDetectorCompat(this, object : GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                Log.d("MainActivity", "onSingleTapConfirmed: ${e.x}, ${e.y}")
                passTouchToRust("click", e.x, e.y)
                return true
            }

            override fun onLongPress(e: MotionEvent) {
                Log.d("MainActivity", "onLongPress: ${e.x}, ${e.y}")
                passTouchToRust("right_click", e.x, e.y)
            }

            override fun onScroll(
                e1: MotionEvent?,
                e2: MotionEvent,
                distanceX: Float,
                distanceY: Float
            ): Boolean {
                Log.d("MainActivity", "onScroll: ${distanceY}")
                passTouchToRust("scroll", 0f, distanceY)
                return true
            }
        })
    }

    override fun onWebViewCreate(webView: WebView) {
        window.decorView.post {
            KeybarPlugin(this, webView).setup()
            setupNativeSurface(webView)
        }
    }

    private fun setupNativeSurface(webView: WebView) {
        val parent = webView.parent as? ViewGroup ?: return
        val index = parent.indexOfChild(webView)
        
        val container = FrameLayout(this)
        container.layoutParams = webView.layoutParams
        
        parent.removeView(webView)
        
        surfaceView = SurfaceView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setOnTouchListener { _, event ->
                mDetector.onTouchEvent(event)
                true
            }
        }
        
        container.addView(surfaceView)
        container.addView(webView)
        
        parent.addView(container, index)
        
        surfaceView?.holder?.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                passSurfaceToRust(holder.surface)
            }
            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}
            override fun surfaceDestroyed(holder: SurfaceHolder) {}
        })
    }

    private external fun passSurfaceToRust(surface: Surface)
    private external fun passTouchToRust(action: String, x: Float, y: Float)

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
