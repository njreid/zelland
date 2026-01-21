package com.zelland.ssh

import com.zelland.model.SSHConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.connection.channel.direct.Session
import net.schmizz.sshj.transport.verification.PromiscuousVerifier
import net.schmizz.sshj.userauth.keyprovider.KeyProvider
import org.bouncycastle.jce.provider.BouncyCastleProvider
import java.io.File
import java.io.IOException
import java.security.Security
import java.util.concurrent.TimeUnit

/**
 * Manages SSH connections and command execution
 */
class SSHConnectionManager {

    private var sshClient: SSHClient? = null
    private var config: SSHConfig? = null

    init {
        // Ensure Bouncy Castle is registered as a security provider
        if (Security.getProvider(BouncyCastleProvider.PROVIDER_NAME) == null) {
            Security.addProvider(BouncyCastleProvider())
        }
    }

    /**
     * Connect to SSH host and authenticate
     */
    @Throws(IOException::class)
    suspend fun connect(sshConfig: SSHConfig) = withContext(Dispatchers.IO) {
        // Close existing connection if any
        disconnect()

        val client = SSHClient()

        try {
            // Configure SSH client
            client.addHostKeyVerifier(PromiscuousVerifier())
            // TODO: Implement proper host key verification for production
            // client.loadKnownHosts()

            // Set connection timeout
            client.connectTimeout = 30000 // 30 seconds
            client.timeout = 10000 // 10 seconds for operations

            // Handle localhost redirection for Android Emulator
            val host = if (sshConfig.host == "localhost" || sshConfig.host == "127.0.0.1") {
                "10.0.2.2"
            } else {
                sshConfig.host
            }

            // Connect to host
            client.connect(host, sshConfig.port)

            // Authenticate
            when (sshConfig.authMethod) {
                SSHConfig.AuthMethod.PASSWORD -> {
                    val password = sshConfig.password
                        ?: throw IllegalArgumentException("Password is required")
                    client.authPassword(sshConfig.username, password)
                }
                SSHConfig.AuthMethod.PRIVATE_KEY -> {
                    val keyPath = sshConfig.privateKeyPath
                        ?: throw IllegalArgumentException("Private key path is required")

                    val keyProvider: KeyProvider = if (sshConfig.privateKeyPassphrase != null) {
                        client.loadKeys(keyPath, sshConfig.privateKeyPassphrase)
                    } else {
                        client.loadKeys(keyPath)
                    }

                    client.authPublickey(sshConfig.username, keyProvider)
                }
            }

            // Verify authentication succeeded
            if (!client.isAuthenticated) {
                throw IOException("Authentication failed")
            }

            // Store successful connection
            sshClient = client
            config = sshConfig

        } catch (e: Exception) {
            // Clean up on failure
            try {
                client.disconnect()
            } catch (ignored: Exception) {
            }
            throw e
        }
    }

    /**
     * Execute a remote command and return output
     */
    @Throws(IOException::class)
    suspend fun executeCommand(command: String, timeoutSeconds: Long = 30): CommandResult =
        withContext(Dispatchers.IO) {
            val client = sshClient ?: throw IllegalStateException("Not connected")

            val session = client.startSession()
            try {
                val cmd = session.exec(command)
                cmd.join(timeoutSeconds, TimeUnit.SECONDS)

                val exitStatus = cmd.exitStatus
                val stdout = cmd.inputStream.bufferedReader().readText()
                val stderr = cmd.errorStream.bufferedReader().readText()

                CommandResult(
                    exitCode = exitStatus,
                    stdout = stdout,
                    stderr = stderr,
                    success = exitStatus == 0
                )
            } finally {
                session.close()
            }
        }

    /**
     * Check if a command exists on the remote system
     */
    @Throws(IOException::class)
    suspend fun commandExists(command: String): Boolean = withContext(Dispatchers.IO) {
        try {
            val result = executeCommand("which $command", timeoutSeconds = 5)
            result.success && result.stdout.isNotBlank()
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Disconnect from SSH host
     */
    fun disconnect() {
        try {
            sshClient?.disconnect()
        } catch (e: Exception) {
            // Ignore errors during disconnect
        } finally {
            sshClient = null
            config = null
        }
    }

    /**
     * Check if currently connected
     */
    fun isConnected(): Boolean {
        return sshClient?.isConnected == true && sshClient?.isAuthenticated == true
    }

    /**
     * Get current config
     */
    fun getConfig(): SSHConfig? = config

    /**
     * Get SSH client for advanced operations
     * Use with caution - direct access to underlying client
     */
    internal fun getClient(): SSHClient? = sshClient

    /**
     * Result of a command execution
     */
    data class CommandResult(
        val exitCode: Int,
        val stdout: String,
        val stderr: String,
        val success: Boolean
    ) {
        fun getOutput(): String = if (stdout.isNotEmpty()) stdout else stderr
    }
}
