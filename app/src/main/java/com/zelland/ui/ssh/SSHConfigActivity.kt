package com.zelland.ui.ssh

import android.os.Bundle
import android.view.View
import android.widget.Button
import android.widget.RadioGroup
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import com.google.android.material.textfield.TextInputEditText
import com.zelland.R
import com.zelland.model.SSHConfig
import com.zelland.viewmodel.TerminalViewModel
import kotlinx.coroutines.launch
import java.util.UUID

class SSHConfigActivity : AppCompatActivity() {

    private lateinit var viewModel: TerminalViewModel

    // Views
    private lateinit var etConnectionName: TextInputEditText
    private lateinit var etHost: TextInputEditText
    private lateinit var etPort: TextInputEditText
    private lateinit var etUsername: TextInputEditText
    private lateinit var rgAuthMethod: RadioGroup
    private lateinit var layoutPassword: View
    private lateinit var layoutPrivateKey: View
    private lateinit var etPassword: TextInputEditText
    private lateinit var etPrivateKeyPath: TextInputEditText
    private lateinit var etKeyPassphrase: TextInputEditText
    private lateinit var btnTestConnection: Button
    private lateinit var btnSave: Button
    private lateinit var tvStatus: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_ssh_config)

        viewModel = ViewModelProvider(this)[TerminalViewModel::class.java]

        setupViews()
        setupListeners()
    }

    private fun setupViews() {
        etConnectionName = findViewById(R.id.etConnectionName)
        etHost = findViewById(R.id.etHost)
        etPort = findViewById(R.id.etPort)
        etUsername = findViewById(R.id.etUsername)
        rgAuthMethod = findViewById(R.id.rgAuthMethod)
        layoutPassword = findViewById(R.id.layoutPassword)
        layoutPrivateKey = findViewById(R.id.layoutPrivateKey)
        etPassword = findViewById(R.id.etPassword)
        etPrivateKeyPath = findViewById(R.id.etPrivateKeyPath)
        etKeyPassphrase = findViewById(R.id.etKeyPassphrase)
        btnTestConnection = findViewById(R.id.btnTestConnection)
        btnSave = findViewById(R.id.btnSave)
        tvStatus = findViewById(R.id.tvStatus)

        // Set up action bar
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        supportActionBar?.title = getString(R.string.ssh_config_title)
    }

    private fun setupListeners() {
        // Auth method toggle
        rgAuthMethod.setOnCheckedChangeListener { _, checkedId ->
            when (checkedId) {
                R.id.rbPassword -> {
                    layoutPassword.visibility = View.VISIBLE
                    layoutPrivateKey.visibility = View.GONE
                }
                R.id.rbPrivateKey -> {
                    layoutPassword.visibility = View.GONE
                    layoutPrivateKey.visibility = View.VISIBLE
                }
            }
        }

        // Test connection button
        btnTestConnection.setOnClickListener {
            testConnection()
        }

        // Save button
        btnSave.setOnClickListener {
            saveConnection()
        }
    }

    private fun buildSSHConfig(): SSHConfig? {
        val name = etConnectionName.text?.toString()?.trim() ?: ""
        val host = etHost.text?.toString()?.trim() ?: ""
        val portStr = etPort.text?.toString()?.trim() ?: "22"
        val username = etUsername.text?.toString()?.trim() ?: ""

        val port = try {
            portStr.toInt()
        } catch (e: NumberFormatException) {
            22
        }

        val authMethod = when (rgAuthMethod.checkedRadioButtonId) {
            R.id.rbPrivateKey -> SSHConfig.AuthMethod.PRIVATE_KEY
            else -> SSHConfig.AuthMethod.PASSWORD
        }

        val password = etPassword.text?.toString()
        val privateKeyPath = etPrivateKeyPath.text?.toString()
        val keyPassphrase = etKeyPassphrase.text?.toString()?.takeIf { it.isNotBlank() }

        val config = SSHConfig(
            id = UUID.randomUUID().toString(),
            name = name,
            host = host,
            port = port,
            username = username,
            authMethod = authMethod,
            password = password,
            privateKeyPath = privateKeyPath,
            privateKeyPassphrase = keyPassphrase
        )

        // Validate
        return when (val result = config.validate()) {
            is SSHConfig.ValidationResult.Success -> config
            is SSHConfig.ValidationResult.Error -> {
                showStatus(result.message, isError = true)
                null
            }
        }
    }

    private fun testConnection() {
        val config = buildSSHConfig() ?: return

        showStatus(getString(R.string.status_testing), isError = false)
        btnTestConnection.isEnabled = false

        lifecycleScope.launch {
            val result = viewModel.testConnection(config)

            when (result) {
                is TerminalViewModel.ConnectionResult.Success -> {
                    showStatus(result.message, isError = false)
                    Toast.makeText(
                        this@SSHConfigActivity,
                        R.string.success_connection_test,
                        Toast.LENGTH_SHORT
                    ).show()
                }
                is TerminalViewModel.ConnectionResult.Error -> {
                    showStatus(result.message, isError = true)
                }
            }

            btnTestConnection.isEnabled = true
        }
    }

    private fun saveConnection() {
        val config = buildSSHConfig() ?: return

        viewModel.addSession(config)

        Toast.makeText(this, R.string.success_connection_saved, Toast.LENGTH_SHORT).show()
        finish()
    }

    private fun showStatus(message: String, isError: Boolean) {
        tvStatus.text = message
        tvStatus.visibility = View.VISIBLE
        tvStatus.setTextColor(
            getColor(if (isError) R.color.status_error else R.color.status_connected)
        )
    }

    override fun onSupportNavigateUp(): Boolean {
        finish()
        return true
    }
}
