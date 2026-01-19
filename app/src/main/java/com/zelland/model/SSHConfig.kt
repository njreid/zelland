package com.zelland.model

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

/**
 * Configuration for SSH connection
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
    val savePassword: Boolean = false
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
        if (username.isBlank()) {
            return ValidationResult.Error("Username cannot be empty")
        }
        if (port !in 1..65535) {
            return ValidationResult.Error("Invalid port number")
        }
        when (authMethod) {
            AuthMethod.PASSWORD -> {
                if (password.isNullOrBlank()) {
                    return ValidationResult.Error("Password cannot be empty")
                }
            }
            AuthMethod.PRIVATE_KEY -> {
                if (privateKeyPath.isNullOrBlank()) {
                    return ValidationResult.Error("Private key path cannot be empty")
                }
            }
        }
        return ValidationResult.Success
    }

    sealed class ValidationResult {
        object Success : ValidationResult()
        data class Error(val message: String) : ValidationResult()
    }
}
