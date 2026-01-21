package com.zelland

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
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
            val config = result.data?.getParcelableExtra<SSHConfig>("ssh_config")
            if (config != null) {
                viewModel.addSession(config)
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
    }

    private fun openSSHConfig() {
        val intent = Intent(this, SSHConfigActivity::class.java)
        sshConfigLauncher.launch(intent)
    }

    override fun onBackPressed() {
        // Handle back press if we're in a terminal session
        // This is a bit simplified; in a production app we'd use a NavHost
        val sessions = viewModel.sessions.value.orEmpty()
        val connectedSession = sessions.find { it.isConnected }
        
        if (connectedSession != null) {
            viewModel.disconnectSession(connectedSession.id)
        } else {
            super.onBackPressed()
        }
    }
}
