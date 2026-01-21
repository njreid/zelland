package com.zelland

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.IntentCompat
import androidx.lifecycle.ViewModelProvider
import com.zelland.model.SSHConfig
import com.zelland.ui.ZellandApp
import com.zelland.ui.ssh.SSHConfigActivity
import com.zelland.viewmodel.TerminalViewModel

class MainActivity : ComponentActivity() {

    private lateinit var viewModel: TerminalViewModel

    private val sshConfigLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            val config = result.data?.let { intent ->
                IntentCompat.getParcelableExtra(intent, "ssh_config", SSHConfig::class.java)
            }
            if (config != null) {
                if (!viewModel.addSession(config)) {
                    Toast.makeText(this, "Session already exists for this host and name", Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        viewModel = ViewModelProvider(this)[TerminalViewModel::class.java]

        setContent {
            ZellandApp(
                viewModel = viewModel,
                onAddSession = { openSSHConfig() }
            )
        }

        // Handle back press using dispatcher instead of deprecated onBackPressed()
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                val sessions = viewModel.sessions.value.orEmpty()
                val connectedSession = sessions.find { it.isConnected }
                
                if (connectedSession != null) {
                    viewModel.disconnectSession(connectedSession.id)
                } else {
                    isEnabled = false
                    onBackPressedDispatcher.onBackPressed()
                    isEnabled = true
                }
            }
        })
    }

    private fun openSSHConfig() {
        val intent = Intent(this, SSHConfigActivity::class.java)
        sshConfigLauncher.launch(intent)
    }
}
