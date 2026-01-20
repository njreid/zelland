package com.zelland

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.floatingactionbutton.FloatingActionButton
import com.zelland.ui.SessionAdapter
import com.zelland.ui.TerminalFragment
import com.zelland.ui.ssh.SSHConfigActivity
import com.zelland.viewmodel.TerminalViewModel

class MainActivity : AppCompatActivity() {

    private lateinit var viewModel: TerminalViewModel
    private lateinit var emptyStateLayout: View
    private lateinit var recyclerView: RecyclerView
    private lateinit var tvConnectionStatus: TextView
    private lateinit var fabAddSession: FloatingActionButton
    private lateinit var fragmentContainer: View
    private lateinit var sessionAdapter: SessionAdapter
    private var currentSessionId: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        viewModel = ViewModelProvider(this)[TerminalViewModel::class.java]

        setupViews()
        observeViewModel()
    }

    private fun setupViews() {
        emptyStateLayout = findViewById(R.id.emptyStateLayout)
        recyclerView = findViewById(R.id.recyclerViewSessions)
        tvConnectionStatus = findViewById(R.id.tvConnectionStatus)
        fabAddSession = findViewById(R.id.fabAddSession)
        fragmentContainer = findViewById(R.id.fragmentContainer)

        // Setup RecyclerView
        sessionAdapter = SessionAdapter(
            sessions = emptyList(),
            onConnect = { session ->
                currentSessionId = session.id
                viewModel.connectSession(session.id)
            },
            onDisconnect = { session ->
                currentSessionId = null
                viewModel.disconnectSession(session.id)
            }
        )

        recyclerView.layoutManager = LinearLayoutManager(this)
        recyclerView.adapter = sessionAdapter

        // Add session button in empty state
        findViewById<View>(R.id.btnAddFirstSession).setOnClickListener {
            openSSHConfig()
        }

        // FAB for adding sessions
        fabAddSession.setOnClickListener {
            openSSHConfig()
        }
    }

    private fun observeViewModel() {
        viewModel.sessions.observe(this) { sessions ->
            if (sessions.isEmpty()) {
                showEmptyState()
            } else {
                showSessions()
                sessionAdapter.updateSessions(sessions)
            }
        }

        viewModel.connectionStatus.observe(this) { status ->
            when (status) {
                is TerminalViewModel.ConnectionStatus.Connecting -> {
                    showStatus(status.sessionName, R.color.status_connecting)
                }
                is TerminalViewModel.ConnectionStatus.Connected -> {
                    showStatus(status.sessionName, R.color.status_connected)
                    // Show terminal fragment when connected
                    currentSessionId?.let { sessionId ->
                        showTerminal(sessionId)
                    }
                    // Auto-hide after 1 second
                    tvConnectionStatus.postDelayed({
                        tvConnectionStatus.visibility = View.GONE
                    }, 1000)
                }
                is TerminalViewModel.ConnectionStatus.Error -> {
                    showStatus(status.message, R.color.status_error)
                }
                is TerminalViewModel.ConnectionStatus.Disconnected -> {
                    tvConnectionStatus.visibility = View.GONE
                    showSessionList()
                }
            }
        }
    }

    private fun showStatus(message: String, colorRes: Int) {
        tvConnectionStatus.text = message
        tvConnectionStatus.setTextColor(getColor(colorRes))
        tvConnectionStatus.visibility = View.VISIBLE
    }

    private fun showEmptyState() {
        emptyStateLayout.visibility = View.VISIBLE
        recyclerView.visibility = View.GONE
    }

    private fun showSessions() {
        emptyStateLayout.visibility = View.GONE
        recyclerView.visibility = View.VISIBLE
    }

    private fun openSSHConfig() {
        val intent = Intent(this, SSHConfigActivity::class.java)
        startActivity(intent)
    }

    private fun showTerminal(sessionId: String) {
        // Hide session list and empty state
        emptyStateLayout.visibility = View.GONE
        recyclerView.visibility = View.GONE
        fabAddSession.visibility = View.GONE

        // Show fragment container
        fragmentContainer.visibility = View.VISIBLE

        // Load TerminalFragment
        val fragment = TerminalFragment.newInstance(sessionId)
        supportFragmentManager.beginTransaction()
            .replace(R.id.fragmentContainer, fragment)
            .addToBackStack("terminal")
            .commit()
    }

    private fun showSessionList() {
        // Hide fragment container
        fragmentContainer.visibility = View.GONE

        // Show session list or empty state
        fabAddSession.visibility = View.VISIBLE
        val sessions = viewModel.sessions.value.orEmpty()
        if (sessions.isEmpty()) {
            showEmptyState()
        } else {
            showSessions()
        }

        // Clear back stack
        supportFragmentManager.popBackStack()
    }

    override fun onBackPressed() {
        // If showing terminal, go back to session list
        if (fragmentContainer.visibility == View.VISIBLE) {
            // Disconnect current session (soft disconnect)
            currentSessionId?.let { sessionId ->
                viewModel.disconnectSession(sessionId)
            }
            currentSessionId = null
            showSessionList()
        } else {
            super.onBackPressed()
        }
    }
}
