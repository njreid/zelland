package com.zelland.ui.ssh

import android.os.Bundle
import android.widget.Button
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import com.google.android.material.textfield.TextInputEditText
import com.zelland.R
import com.zelland.model.SSHConfig
import java.util.UUID

class SSHConfigActivity : AppCompatActivity() {

    // Views
    private lateinit var etConnectionName: TextInputEditText
    private lateinit var etHost: TextInputEditText
    private lateinit var etSessionName: TextInputEditText
    private lateinit var btnSave: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_ssh_config)

        setupViews()
        setupListeners()
    }

    private fun setupViews() {
        etConnectionName = findViewById(R.id.etConnectionName)
        etHost = findViewById(R.id.etHost)
        etSessionName = findViewById(R.id.etSessionName)
        btnSave = findViewById(R.id.btnSave)

        // Set up action bar
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        supportActionBar?.title = getString(R.string.ssh_config_title)
    }

    private fun setupListeners() {
        // Save button
        btnSave.setOnClickListener {
            saveConnection()
        }
    }

    private fun buildSSHConfig(): SSHConfig? {
        val name = etConnectionName.text?.toString()?.trim() ?: ""
        val host = etHost.text?.toString()?.trim() ?: ""
        val sessionNameInput = etSessionName.text?.toString()?.trim() ?: ""

        if (host.isBlank()) {
            Toast.makeText(this, "Host cannot be empty", Toast.LENGTH_SHORT).show()
            return null
        }

        // Derive session name if empty, otherwise use input
        // "My Session" -> "my-session"
        val rawSessionName = if (sessionNameInput.isBlank()) name else sessionNameInput
        val formattedSessionName = rawSessionName.lowercase()
            .replace(Regex("[^a-z0-9]+"), "-")
            .trim('-')

        return SSHConfig(
            id = UUID.randomUUID().toString(),
            name = name,
            host = host,
            port = 22,
            username = "dummy",
            authMethod = SSHConfig.AuthMethod.PASSWORD,
            password = null,
            privateKeyPath = null,
            privateKeyPassphrase = null,
            zellijSessionName = formattedSessionName
        )
    }

    private fun saveConnection() {
        val config = buildSSHConfig() ?: return

        // Pass the config back to MainActivity via an Intent result.
        val intent = android.content.Intent()
        intent.putExtra("ssh_config", config)
        setResult(RESULT_OK, intent)

        Toast.makeText(this, R.string.success_connection_saved, Toast.LENGTH_SHORT).show()
        finish()
    }

    override fun onSupportNavigateUp(): Boolean {
        finish()
        return true
    }
}
