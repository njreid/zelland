package com.zelland.viewmodel

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.zelland.model.SSHConfig
import com.zelland.model.TerminalSession
import com.zelland.ssh.SSHConnectionManager
import com.zelland.zellij.ZellijManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException
import java.util.UUID

/**
 * ViewModel for managing terminal sessions
 */
class TerminalViewModel : ViewModel() {

    private val sshManagers = mutableMapOf<String, SSHConnectionManager>()
    private val zellijManagers = mutableMapOf<String, ZellijManager>()

    private val _sessions = MutableLiveData<List<TerminalSession>>(emptyList())
    val sessions: LiveData<List<TerminalSession>> = _sessions

    private val _activeSessionIndex = MutableLiveData<Int>(0)
    val activeSessionIndex: LiveData<Int> = _activeSessionIndex

    private val _connectionStatus = MutableLiveData<ConnectionStatus>()
    val connectionStatus: LiveData<ConnectionStatus> = _connectionStatus

    /**
     * Test SSH connection without creating a session
     */
    suspend fun testConnection(config: SSHConfig): ConnectionResult =
        withContext(Dispatchers.IO) {
            val manager = SSHConnectionManager()
            try {
                manager.connect(config)
                val result = manager.executeCommand("echo 'Connection successful'")
                manager.disconnect()

                if (result.success) {
                    ConnectionResult.Success("Connection successful!")
                } else {
                    ConnectionResult.Error("Connection test failed: ${result.stderr}")
                }
            } catch (e: IOException) {
                ConnectionResult.Error("Connection failed: ${e.message}")
            } catch (e: Exception) {
                ConnectionResult.Error("Unexpected error: ${e.message}")
            } finally {
                manager.disconnect()
            }
        }

    /**
     * Add a new session with the given SSH config
     */
    fun addSession(config: SSHConfig) {
        val sessionId = UUID.randomUUID().toString()
        val title = config.name.ifBlank { "${config.username}@${config.host}" }

        val newSession = TerminalSession(
            id = sessionId,
            title = title,
            sshConfig = config,
            isConnected = false
        )

        val currentSessions = _sessions.value.orEmpty().toMutableList()
        currentSessions.add(newSession)
        _sessions.value = currentSessions

        // Auto-select the new session
        _activeSessionIndex.value = currentSessions.size - 1
    }

    /**
     * Remove a session
     */
    fun removeSession(sessionId: String) {
        // Disconnect if connected
        disconnectSession(sessionId)

        val currentSessions = _sessions.value.orEmpty().toMutableList()
        currentSessions.removeAll { it.id == sessionId }
        _sessions.value = currentSessions

        // Adjust active index if needed
        val activeIndex = _activeSessionIndex.value ?: 0
        if (activeIndex >= currentSessions.size && currentSessions.isNotEmpty()) {
            _activeSessionIndex.value = currentSessions.size - 1
        }
    }

    /**
     * Connect to a session
     * Full implementation with Zellij web integration
     */
    fun connectSession(sessionId: String) {
        viewModelScope.launch {
            val session = _sessions.value?.find { it.id == sessionId } ?: return@launch

            _connectionStatus.postValue(ConnectionStatus.Connecting(session.title))

            try {
                // Step 1: Connect via SSH
                _connectionStatus.postValue(
                    ConnectionStatus.Connecting("Connecting to ${session.sshConfig.host}...")
                )
                val sshManager = SSHConnectionManager()
                sshManager.connect(session.sshConfig)

                // Store SSH manager
                sshManagers[sessionId] = sshManager

                // Step 2: Create Zellij manager
                val zellijManager = ZellijManager(sshManager)
                zellijManagers[sessionId] = zellijManager

                // Step 3: Check if Zellij is installed
                _connectionStatus.postValue(
                    ConnectionStatus.Connecting("Checking for Zellij...")
                )
                if (!zellijManager.isZellijInstalled()) {
                    throw ZellijManager.ZellijException.NotInstalled(
                        "Zellij not found on ${session.sshConfig.host}"
                    )
                }

                // Get version info
                val version = zellijManager.getZellijVersion()
                android.util.Log.i("TerminalViewModel", "Zellij version: $version")

                // Step 4: Start Zellij web server
                _connectionStatus.postValue(
                    ConnectionStatus.Connecting("Starting Zellij web server...")
                )
                val remotePort = zellijManager.startZellijWeb()

                android.util.Log.i("TerminalViewModel",
                    "Zellij web started on port $remotePort")

                // Step 5: Get Tailscale IP
                _connectionStatus.postValue(
                    ConnectionStatus.Connecting("Getting Tailscale IP...")
                )
                val tailscaleIP = zellijManager.getTailscaleIP()
                    ?: session.sshConfig.host  // Fallback to SSH host if Tailscale not available

                android.util.Log.i("TerminalViewModel", "Tailscale IP: $tailscaleIP")

                // Step 6: Get or create authentication token
                _connectionStatus.postValue(
                    ConnectionStatus.Connecting("Getting authentication token...")
                )
                val authToken = zellijManager.getOrCreateAuthToken()

                android.util.Log.d("TerminalViewModel", "Got Zellij auth token")

                // Step 7: Build Zellij web URL
                val zellijUrl = "http://$tailscaleIP:$remotePort/${session.zellijSessionName}?token=$authToken"

                android.util.Log.i("TerminalViewModel", "Zellij URL: http://$tailscaleIP:$remotePort/${session.zellijSessionName}")

                // Step 8: Update session with URL
                updateSession(session.copy(
                    isConnected = true,
                    localUrl = zellijUrl,
                    lastConnected = System.currentTimeMillis()
                ))

                _connectionStatus.postValue(
                    ConnectionStatus.Connected("Connected to ${session.title}")
                )

            } catch (e: ZellijManager.ZellijException.NotInstalled) {
                _connectionStatus.postValue(
                    ConnectionStatus.Error(e.message ?: "Zellij not installed")
                )
                // Clean up
                sshManagers[sessionId]?.disconnect()
                sshManagers.remove(sessionId)
                zellijManagers.remove(sessionId)

            } catch (e: ZellijManager.ZellijException) {
                _connectionStatus.postValue(
                    ConnectionStatus.Error("Zellij error: ${e.message}")
                )
                // Clean up
                sshManagers[sessionId]?.disconnect()
                sshManagers.remove(sessionId)
                zellijManagers.remove(sessionId)

            } catch (e: IOException) {
                _connectionStatus.postValue(
                    ConnectionStatus.Error("SSH connection failed: ${e.message}")
                )
                // Clean up
                sshManagers[sessionId]?.disconnect()
                sshManagers.remove(sessionId)
                zellijManagers.remove(sessionId)

            } catch (e: Exception) {
                _connectionStatus.postValue(
                    ConnectionStatus.Error("Unexpected error: ${e.message}")
                )
                // Clean up
                sshManagers[sessionId]?.disconnect()
                sshManagers.remove(sessionId)
                zellijManagers.remove(sessionId)
            }
        }
    }

    /**
     * Disconnect a session (soft disconnect - keeps Zellij running)
     */
    fun disconnectSession(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            // Just close SSH connection, leave Zellij running
            sshManagers[sessionId]?.disconnect()
            sshManagers.remove(sessionId)
            zellijManagers.remove(sessionId)

            val session = _sessions.value?.find { it.id == sessionId }
            if (session != null) {
                updateSession(session.copy(isConnected = false, localUrl = null))
            }

            withContext(Dispatchers.Main) {
                _connectionStatus.value = ConnectionStatus.Disconnected
            }
        }
    }

    /**
     * Disconnect and stop Zellij web server (hard disconnect)
     */
    fun killSession(sessionId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                // Stop Zellij web server
                zellijManagers[sessionId]?.stopZellijWeb()
            } catch (e: Exception) {
                android.util.Log.e("TerminalViewModel", "Error stopping Zellij", e)
            }

            // Close SSH connection
            sshManagers[sessionId]?.disconnect()
            sshManagers.remove(sessionId)
            zellijManagers.remove(sessionId)

            val session = _sessions.value?.find { it.id == sessionId }
            if (session != null) {
                updateSession(session.copy(isConnected = false, localUrl = null))
            }

            withContext(Dispatchers.Main) {
                _connectionStatus.value = ConnectionStatus.Disconnected
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
        }
    }

    override fun onCleared() {
        super.onCleared()
        // Clean up all SSH connections
        sshManagers.values.forEach { it.disconnect() }
        sshManagers.clear()
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
