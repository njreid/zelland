package com.zelland.zellij

import com.zelland.ssh.SSHConnectionManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.io.IOException

/**
 * Manages Zellij web server lifecycle on remote hosts
 */
class ZellijManager(private val sshManager: SSHConnectionManager) {

    companion object {
        private const val DEFAULT_ZELLIJ_PORT = 8082
        private const val STARTUP_WAIT_MS = 2000L
        private const val MAX_STARTUP_RETRIES = 5
        private const val RETRY_DELAY_MS = 500L
    }

    /**
     * Check if Zellij is installed on the remote host
     */
    @Throws(IOException::class)
    suspend fun isZellijInstalled(): Boolean = withContext(Dispatchers.IO) {
        sshManager.commandExists("zellij")
    }

    /**
     * Get Zellij version from remote host
     */
    @Throws(IOException::class)
    suspend fun getZellijVersion(): String? = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand("zellij --version", timeoutSeconds = 5)
            if (result.success) {
                // Output format: "zellij 0.43.0"
                result.stdout.trim().substringAfter("zellij").trim()
            } else {
                null
            }
        } catch (e: Exception) {
            null
        }
    }

    /**
     * Check if Zellij web server is currently running
     */
    @Throws(IOException::class)
    suspend fun isZellijWebRunning(): Boolean = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "pgrep -f 'zellij web' | head -1",
                timeoutSeconds = 5
            )
            result.success && result.stdout.trim().isNotEmpty()
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Get the PID of the running Zellij web server
     */
    @Throws(IOException::class)
    suspend fun getZellijWebPid(): Int? = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "pgrep -f 'zellij web' | head -1",
                timeoutSeconds = 5
            )
            if (result.success && result.stdout.trim().isNotEmpty()) {
                result.stdout.trim().toIntOrNull()
            } else {
                null
            }
        } catch (e: Exception) {
            null
        }
    }

    /**
     * Get Tailscale IPv4 address of the remote host
     * Returns null if Tailscale is not installed or not connected
     */
    @Throws(IOException::class)
    suspend fun getTailscaleIP(): String? = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand("tailscale ip -4 2>/dev/null", timeoutSeconds = 5)
            if (result.success && result.stdout.trim().isNotEmpty()) {
                val ip = result.stdout.trim()
                android.util.Log.d("ZellijManager", "Tailscale IP: $ip")
                ip
            } else {
                android.util.Log.w("ZellijManager", "Failed to get Tailscale IP: ${result.stderr}")
                null
            }
        } catch (e: Exception) {
            android.util.Log.w("ZellijManager", "Error getting Tailscale IP", e)
            null
        }
    }

    /**
     * Get or create an authentication token for Zellij web
     * Uses 'zellij web --create-token' to generate a new token
     */
    @Throws(IOException::class, ZellijException::class)
    suspend fun getOrCreateAuthToken(): String = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "zellij web --create-token",
                timeoutSeconds = 10
            )

            if (result.success) {
                // The output might contain ANSI escape codes, so we need to clean it up
                // Example output: "Created token successfully\n\ntoken_1: 40cfd772-e052-43a0-8acf-e64b1b8825fb"
                // Or sometimes just the token if piped, but let's be robust.
                
                val rawOutput = result.stdout
                
                // Remove ANSI escape codes
                val cleanOutput = rawOutput.replace(Regex("\u001B\\[[;\\d]*m"), "")
                
                // Look for the token pattern (UUID-like)
                // Or just take the last line if it looks like a token
                // The log shows: "token_1: 40cfd772-e052-43a0-8acf-e64b1b8825fb"
                
                val tokenMatch = Regex("token_\\d+:\\s*([a-f0-9\\-]+)").find(cleanOutput)
                if (tokenMatch != null) {
                    val token = tokenMatch.groupValues[1]
                    android.util.Log.d("ZellijManager", "Extracted Zellij auth token: $token")
                    return@withContext token
                }
                
                // Fallback: try to find just a UUID in the output
                val uuidMatch = Regex("[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}").find(cleanOutput)
                if (uuidMatch != null) {
                    val token = uuidMatch.value
                    android.util.Log.d("ZellijManager", "Found UUID token: $token")
                    return@withContext token
                }

                // If output is just the token (no text)
                val trimmed = cleanOutput.trim()
                if (trimmed.isNotEmpty() && trimmed.length > 20) {
                     return@withContext trimmed
                }

                throw ZellijException.StartupFailed(
                    "Failed to parse auth token from output: $cleanOutput"
                )
            } else {
                throw ZellijException.StartupFailed(
                    "Failed to create auth token: ${result.stderr}"
                )
            }
        } catch (e: ZellijException) {
            throw e
        } catch (e: Exception) {
            throw ZellijException.StartupFailed(
                "Error creating auth token: ${e.message}"
            )
        }
    }

    /**
     * Start Zellij web server on remote host
     * Returns the port number the server is listening on
     */
    @Throws(IOException::class, ZellijException::class)
    suspend fun startZellijWeb(): Int = withContext(Dispatchers.IO) {
        // Check if Zellij is installed
        if (!isZellijInstalled()) {
            throw ZellijException.NotInstalled(
                "Zellij is not installed on the remote host. " +
                "Install with: curl -L zellij.dev/install.sh | bash"
            )
        }

        // Check version (optional warning)
        val version = getZellijVersion()
        if (version != null && !isVersionSupported(version)) {
            // Just log warning, don't fail
            android.util.Log.w("ZellijManager", "Zellij version $version may not support web client")
        }

        // Check if already running
        if (isZellijWebRunning()) {
            android.util.Log.i("ZellijManager", "Zellij web already running")
            return@withContext DEFAULT_ZELLIJ_PORT
        }

        // Start Zellij web in background
        // Use nohup and redirect output to a log file
        // IMPORTANT: We need to ensure the process detaches properly so SSH can close
        val startCommand = "nohup zellij web > /tmp/zellij-web.log 2>&1 &"

        val result = sshManager.executeCommand(startCommand, timeoutSeconds = 10)
        if (!result.success) {
            throw ZellijException.StartupFailed(
                "Failed to start Zellij web: ${result.stderr}"
            )
        }

        android.util.Log.d("ZellijManager", "Started Zellij web command")

        // Wait for server to start
        delay(STARTUP_WAIT_MS)

        // Verify it's running
        var retries = 0
        while (retries < MAX_STARTUP_RETRIES) {
            if (isZellijWebRunning()) {
                android.util.Log.i("ZellijManager", "Zellij web started successfully")
                return@withContext DEFAULT_ZELLIJ_PORT
            }
            delay(RETRY_DELAY_MS)
            retries++
        }

        // Check logs for error
        val logResult = sshManager.executeCommand("tail -20 /tmp/zellij-web.log", timeoutSeconds = 5)
        throw ZellijException.StartupFailed(
            "Zellij web failed to start. Logs:\n${logResult.stdout}"
        )
    }

    /**
     * Stop Zellij web server
     */
    @Throws(IOException::class)
    suspend fun stopZellijWeb(): Boolean = withContext(Dispatchers.IO) {
        try {
            // Kill the process
            val result = sshManager.executeCommand(
                "pkill -f 'zellij web'",
                timeoutSeconds = 5
            )

            // Give it a moment to terminate
            delay(500)

            // Verify it stopped
            val stillRunning = isZellijWebRunning()
            if (stillRunning) {
                android.util.Log.w("ZellijManager", "Zellij web may still be running after pkill")
            }

            !stillRunning
        } catch (e: Exception) {
            android.util.Log.e("ZellijManager", "Error stopping Zellij web", e)
            false
        }
    }

    /**
     * List all Zellij sessions on the remote host
     */
    @Throws(IOException::class)
    suspend fun listSessions(): List<String> = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "zellij list-sessions 2>/dev/null || echo ''",
                timeoutSeconds = 10
            )

            if (result.success && result.stdout.isNotEmpty()) {
                // Parse session names from output
                // Format varies by Zellij version, but typically one session per line
                result.stdout.lines()
                    .filter { it.isNotBlank() }
                    .map { it.trim() }
            } else {
                emptyList()
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * Check if a specific Zellij session exists
     */
    @Throws(IOException::class)
    suspend fun sessionExists(sessionName: String): Boolean = withContext(Dispatchers.IO) {
        val sessions = listSessions()
        sessions.any { it.contains(sessionName) }
    }

    /**
     * Kill a specific Zellij session
     */
    @Throws(IOException::class)
    suspend fun killSession(sessionName: String): Boolean = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "zellij delete-session $sessionName",
                timeoutSeconds = 10
            )
            result.success
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Get Zellij web server logs
     */
    @Throws(IOException::class)
    suspend fun getLogs(lines: Int = 50): String = withContext(Dispatchers.IO) {
        try {
            val result = sshManager.executeCommand(
                "tail -$lines /tmp/zellij-web.log 2>/dev/null || echo 'No logs available'",
                timeoutSeconds = 5
            )
            result.stdout
        } catch (e: Exception) {
            "Error retrieving logs: ${e.message}"
        }
    }

    /**
     * Check if Zellij version supports web client
     * Web client was added in 0.43.0
     */
    private fun isVersionSupported(version: String): Boolean {
        return try {
            val versionParts = version.split(".")
            if (versionParts.size < 2) return false

            val major = versionParts[0].toIntOrNull() ?: return false
            val minor = versionParts[1].toIntOrNull() ?: return false

            // Web client added in 0.43.0
            when {
                major > 0 -> true
                major == 0 && minor >= 43 -> true
                else -> false
            }
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Exceptions specific to Zellij operations
     */
    sealed class ZellijException(message: String) : Exception(message) {
        class NotInstalled(message: String) : ZellijException(message)
        class StartupFailed(message: String) : ZellijException(message)
        class VersionTooOld(message: String) : ZellijException(message)
    }
}
