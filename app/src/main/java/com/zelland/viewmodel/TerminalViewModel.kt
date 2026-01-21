package com.zelland.viewmodel

import android.app.Application
import android.content.Context
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import com.zelland.model.SSHConfig
import com.zelland.model.TerminalSession
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.util.UUID
import javax.net.ssl.HttpsURLConnection

/**
 * ViewModel for managing terminal sessions
 */
class TerminalViewModel(application: Application) : AndroidViewModel(application) {

    private val _sessions = MutableLiveData<List<TerminalSession>>(emptyList())
    val sessions: LiveData<List<TerminalSession>> = _sessions

    private val _activeSessionIndex = MutableLiveData<Int>(0)
    val activeSessionIndex: LiveData<Int> = _activeSessionIndex

    private val _connectionStatus = MutableLiveData<ConnectionStatus>()
    val connectionStatus: LiveData<ConnectionStatus> = _connectionStatus

    private val gson = Gson()
    private val sharedPreferences = application.getSharedPreferences("zelland_prefs", Context.MODE_PRIVATE)

    init {
        loadSessions()
    }

    private fun loadSessions() {
        val json = sharedPreferences.getString("sessions", null)
        if (json != null) {
            val type = object : TypeToken<List<TerminalSession>>() {}.type
            val loadedSessions: List<TerminalSession> = gson.fromJson(json, type)
            _sessions.value = loadedSessions
        }
    }

    private fun saveSessions(sessionsToSave: List<TerminalSession>) {
        val json = gson.toJson(sessionsToSave)
        sharedPreferences.edit().putString("sessions", json).apply()
    }

    /**
     * Test connection (now just a ping check since we removed SSH)
     */
    suspend fun testConnection(config: SSHConfig): ConnectionResult =
        withContext(Dispatchers.IO) {
            // Simple reachability check
            try {
                val host = if (config.host == "localhost" || config.host == "127.0.0.1") {
                    "10.0.2.2"
                } else {
                    config.host
                }
                // Check root or specifically the session if provided
                val path = if (!config.zellijSessionName.isNullOrBlank()) "/${config.zellijSessionName}" else ""
                val url = "https://$host:8082$path" 
                
                if (checkDirectConnection(url)) {
                    ConnectionResult.Success("Connection successful!")
                } else {
                    ConnectionResult.Error("Could not reach Zellij server at $url")
                }
            } catch (e: Exception) {
                ConnectionResult.Error("Connection failed: ${e.message}")
            }
        }

    /**
     * Add a new session with the given SSH config
     */
    fun addSession(config: SSHConfig): Boolean {
        // Check for duplicates
        val currentSessions = _sessions.value.orEmpty().toMutableList()
        val isDuplicate = currentSessions.any { 
            it.sshConfig.host == config.host && it.zellijSessionName == config.zellijSessionName 
        }
        
        if (isDuplicate) {
            return false
        }

        val sessionId = UUID.randomUUID().toString()
        val title = config.name.ifBlank { config.host }
        val zellijName = config.zellijSessionName ?: "session-${sessionId.take(8)}"

        val newSession = TerminalSession(
            id = sessionId,
            title = title,
            sshConfig = config,
            zellijSessionName = zellijName,
            isConnected = false
        )

        currentSessions.add(newSession)
        _sessions.value = currentSessions

        // Auto-select the new session
        _activeSessionIndex.value = currentSessions.size - 1
        
        saveSessions(currentSessions)
        return true
    }

    /**
     * Remove a session
     */
    fun removeSession(sessionId: String) {
        disconnectSession(sessionId)

        val currentSessions = _sessions.value.orEmpty().toMutableList()
        currentSessions.removeAll { it.id == sessionId }
        _sessions.value = currentSessions

        // Adjust active index if needed
        val activeIndex = _activeSessionIndex.value ?: 0
        if (activeIndex >= currentSessions.size && currentSessions.isNotEmpty()) {
            _activeSessionIndex.value = currentSessions.size - 1
        }
        
        saveSessions(currentSessions)
    }

    /**
     * Connect to a session
     * Now assumes Zellij is already running and connects directly via HTTPS
     */
    fun connectSession(sessionId: String) {
        viewModelScope.launch {
            val session = _sessions.value?.find { it.id == sessionId } ?: return@launch

            _connectionStatus.postValue(ConnectionStatus.Connecting(session.title))

            // Determine host (handle localhost for emulator)
            val host = if (session.sshConfig.host == "localhost" || session.sshConfig.host == "127.0.0.1") {
                "10.0.2.2"
            } else {
                session.sshConfig.host
            }
            
            val port = 8082 // Default Zellij port
            
            // Build URL with session name as route
            val url = "https://$host:$port/${session.zellijSessionName}" 
            
            android.util.Log.i("TerminalViewModel", "Connecting to $url")

            if (checkDirectConnection(url)) {
                android.util.Log.i("TerminalViewModel", "Connection successful")
                updateSession(session.copy(
                    isConnected = true,
                    localUrl = url,
                    lastConnected = System.currentTimeMillis()
                ))
                _connectionStatus.postValue(ConnectionStatus.Connected("Connected to ${session.title}"))
            } else {
                android.util.Log.e("TerminalViewModel", "Connection failed")
                _connectionStatus.postValue(ConnectionStatus.Error("Could not reach server at $url"))
            }
        }
    }
    
    /**
     * Check if a direct HTTPS connection is possible
     */
    private suspend fun checkDirectConnection(urlStr: String): Boolean = withContext(Dispatchers.IO) {
        try {
            val url = URL(urlStr)
            val connection = url.openConnection() as HttpsURLConnection
            connection.requestMethod = "HEAD"
            connection.connectTimeout = 2000 // 2 seconds timeout
            connection.readTimeout = 2000
            
            // Trust all certificates for development/self-signed certs
            val trustAllCerts = arrayOf<javax.net.ssl.TrustManager>(object : javax.net.ssl.X509TrustManager {
                override fun getAcceptedIssuers(): Array<java.security.cert.X509Certificate>? = null
                override fun checkClientTrusted(certs: Array<java.security.cert.X509Certificate>, authType: String) {}
                override fun checkServerTrusted(certs: Array<java.security.cert.X509Certificate>, authType: String) {}
            })
            
            val sc = javax.net.ssl.SSLContext.getInstance("SSL")
            sc.init(null, trustAllCerts, java.security.SecureRandom())
            connection.sslSocketFactory = sc.socketFactory
            connection.hostnameVerifier = javax.net.ssl.HostnameVerifier { _, _ -> true }
            
            val responseCode = connection.responseCode
            android.util.Log.d("TerminalViewModel", "Direct connection check: $responseCode")
            
            // 200 OK or 401/403 (means it's there but needs auth)
            responseCode in 200..399 || responseCode == 401 || responseCode == 403
        } catch (e: Exception) {
            android.util.Log.d("TerminalViewModel", "Direct connection check failed: ${e.message}")
            false
        }
    }

    /**
     * Disconnect a session
     */
    fun disconnectSession(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val session = _sessions.value?.find { it.id == sessionId }
            if (session != null) {
                // Use withContext to update LiveData on the main thread
                withContext(Dispatchers.Main) {
                    updateSession(session.copy(isConnected = false, localUrl = null))
                    _connectionStatus.value = ConnectionStatus.Disconnected
                }
            } else {
                withContext(Dispatchers.Main) {
                    _connectionStatus.value = ConnectionStatus.Disconnected
                }
            }
        }
    }

    /**
     * Set active session index
     */
    fun setActiveSessionIndex(index: Int) {
        _activeSessionIndex.value = index
    }

    /**
     * Update an existing session
     */
    private fun updateSession(updatedSession: TerminalSession) {
        val currentSessions = _sessions.value.orEmpty().toMutableList()
        val index = currentSessions.indexOfFirst { it.id == updatedSession.id }
        if (index != -1) {
            currentSessions[index] = updatedSession
            _sessions.value = currentSessions
            saveSessions(currentSessions)
        }
    }

    override fun onCleared() {
        super.onCleared()
    }

    /**
     * Connection result for testing
     */
    sealed class ConnectionResult {
        data class Success(val message: String) : ConnectionResult()
        data class Error(val message: String) : ConnectionResult()
    }

    /**
     * Connection status for UI updates
     */
    sealed class ConnectionStatus {
        data class Connecting(val sessionName: String) : ConnectionStatus()
        data class Connected(val sessionName: String) : ConnectionStatus()
        data class Error(val message: String) : ConnectionStatus()
        object Disconnected : ConnectionStatus()
    }
}
