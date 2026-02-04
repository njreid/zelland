package com.zelland.daemon

import android.util.Log
import com.zelland.proto.Envelope
import com.zelland.proto.KeepAlive
import okhttp3.*
import okio.ByteString
import java.security.SecureRandom
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.*

class DaemonConnectionManager(
    private val host: String,
    private val port: Int,
    private val psk: String?,
    private val listener: DaemonListener
) {
    interface DaemonListener {
        fun onMessageReceived(envelope: Envelope)
        fun onStatusChanged(connected: Boolean)
        fun onError(error: String)
    }

    private var client: OkHttpClient? = null
    private var webSocket: WebSocket? = null
    private val tag = "DaemonConnMgr"
    private var isClosing = false

    fun connect() {
        if (webSocket != null) return
        isClosing = false

        val clientBuilder = OkHttpClient.Builder()
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .connectTimeout(10, TimeUnit.SECONDS)
            .pingInterval(30, TimeUnit.SECONDS)

        setupUnsafeSsl(clientBuilder)

        val client = clientBuilder.build()

        val protocol = "ws"
        val url = "$protocol://$host:$port/ws"
        Log.i(tag, "Connecting to Daemon WebSocket: $url")

        val requestBuilder = Request.Builder()
            .url(url)

        psk?.let {
            requestBuilder.addHeader("X-Zelland-PSK", it)
        }

        webSocket = client?.newWebSocket(requestBuilder.build(), object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                Log.i(tag, "WebSocket opened successfully to $url")
                listener.onStatusChanged(true)
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                try {
                    val envelope = Envelope.parseFrom(bytes.toByteArray())
                    handleEnvelope(envelope)
                } catch (e: Exception) {
                    Log.e(tag, "Failed to parse protobuf from $url", e)
                }
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                Log.i(tag, "WebSocket closing ($url): $reason")
                listener.onStatusChanged(false)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                Log.e(tag, "WebSocket failure ($url): ${t.message}")
                listener.onError(t.message ?: "Unknown error")
                listener.onStatusChanged(false)
                this@DaemonConnectionManager.webSocket = null
                
                // Attempt reconnect if not explicitly closing
                if (!isClosing) {
                    Log.i(tag, "Attempting to reconnect to $url in 5 seconds...")
                    // Using a simple delay for now, in a real app we might use a handler or coroutine
                    Thread {
                        try {
                            Thread.sleep(5000)
                            if (!isClosing) connect()
                        } catch (e: InterruptedException) {
                            Log.e(tag, "Reconnect thread interrupted", e)
                        }
                    }.start()
                }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                Log.i(tag, "WebSocket closed ($url)")
                listener.onStatusChanged(false)
                this@DaemonConnectionManager.webSocket = null
            }
        })
    }

    private fun handleEnvelope(envelope: Envelope) {
        when (envelope.payloadCase) {
            Envelope.PayloadCase.PING -> {
                Log.d(tag, "Received Ping, responding with Pong")
                sendEnvelope(Envelope.newBuilder()
                    .setPing(KeepAlive.newBuilder().setTimestamp(System.currentTimeMillis()))
                    .build())
            }
            else -> {
                listener.onMessageReceived(envelope)
            }
        }
    }

    fun sendEnvelope(envelope: Envelope) {
        webSocket?.send(ByteString.of(*envelope.toByteArray()))
    }

    fun disconnect() {
        isClosing = true
        webSocket?.close(1000, "User disconnect")
        webSocket = null
        client = null
    }

    private fun setupUnsafeSsl(builder: OkHttpClient.Builder) {
        try {
            val trustAllCerts = arrayOf<TrustManager>(object : X509TrustManager {
                override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
                override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {}
                override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
            })

            val sslContext = SSLContext.getInstance("SSL")
            sslContext.init(null, trustAllCerts, SecureRandom())
            val sslSocketFactory = sslContext.socketFactory

            builder.sslSocketFactory(sslSocketFactory, trustAllCerts[0] as X509TrustManager)
            builder.hostnameVerifier { _, _ -> true }
        } catch (e: Exception) {
            Log.e(tag, "Failed to setup unsafe SSL", e)
        }
    }
}
