package com.zelland.model

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

/**
 * Configuration for a connection
 */
@Parcelize
data class SSHConfig(
    val id: String,
    val name: String,
    val host: String,
    val port: Int = 22,
    val username: String,
    val authMethod: AuthMethod = AuthMethod.PASSWORD,
    val password: String? = null,
    val privateKeyPath: String? = null,
    val privateKeyPassphrase: String? = null,
    val savePassword: Boolean = false,
    val zellijSessionName: String? = null,
    val daemonPort: Int = 8083,
    val daemonPsk: String? = null
) : Parcelable {

    enum class AuthMethod {
        PASSWORD,
        PRIVATE_KEY
    }

    /**
     * Validate configuration
     */
    fun validate(): ValidationResult {
        if (host.isBlank()) {
            return ValidationResult.Error("Host cannot be empty")
        }
        return ValidationResult.Success
    }

    sealed class ValidationResult {
        object Success : ValidationResult()
        data class Error(val message: String) : ValidationResult()
    }
}
