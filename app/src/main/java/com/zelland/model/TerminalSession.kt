package com.zelland.model

import java.util.UUID

/**
 * Represents a terminal session connected to a remote Zellij server
 */
data class TerminalSession(
    val id: String = UUID.randomUUID().toString(),
    val title: String,
    val sshConfig: SSHConfig,

    // Persistent Zellij session name
    // This is the key to reconnecting to the same session
    val zellijSessionName: String,

    val isConnected: Boolean = false,
    val localUrl: String? = null,
    
    // Store the last known auth token to try reconnecting directly
    val lastAuthToken: String? = null,

    // Track last connection time
    val lastConnected: Long = System.currentTimeMillis()
) {
    /**
     * Get display name for this session
     */
    fun getDisplayName(): String {
        return "$title (${sshConfig.host})"
    }

    /**
     * Get connection status for display
     */
    fun getStatusText(): String {
        return when {
            isConnected -> "Connected"
            lastConnected > 0 -> "Disconnected"
            else -> "Not connected"
        }
    }
}
