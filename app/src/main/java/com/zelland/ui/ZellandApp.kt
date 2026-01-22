package com.zelland.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import com.zelland.model.TerminalSession
import com.zelland.viewmodel.TerminalViewModel
import com.zelland.ui.theme.ZellandTheme

sealed class Screen {
    object Main : Screen()
    data class SessionView(val sessionId: String) : Screen()
}

@Composable
fun ZellandApp(
    viewModel: TerminalViewModel,
    onAddSession: () -> Unit
) {
    var currentScreen by remember { mutableStateOf<Screen>(Screen.Main) }
    val sessions: List<TerminalSession> by viewModel.sessions.observeAsState(emptyList())
    val connectionStatus by viewModel.connectionStatus.observeAsState()

    // Handle back press or screen transitions
    LaunchedEffect(connectionStatus) {
        if (connectionStatus is TerminalViewModel.ConnectionStatus.Connected) {
            val session = sessions.find { it.isConnected }
            if (session != null) {
                currentScreen = Screen.SessionView(session.id)
            }
        } else if (connectionStatus is TerminalViewModel.ConnectionStatus.Disconnected) {
            currentScreen = Screen.Main
        }
    }

    ZellandTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = Color(0xFF1E1E1E)) {
            when (val screen = currentScreen) {
                is Screen.Main -> {
                    MainScreen(
                        viewModel = viewModel,
                        onAddSession = onAddSession,
                        onConnect = { session ->
                            viewModel.connectSession(session.id)
                        }
                    )
                }
                is Screen.SessionView -> {
                    val session = sessions.find { it.id == screen.sessionId }
                    if (session != null) {
                        if (session.activeView == TerminalSession.ActiveView.Terminal) {
                            TerminalScreen(
                                session = session,
                                viewModel = viewModel,
                                onModifierUsed = { /* Handled in TerminalScreen */ }
                            )
                        } else if (session.activeView == TerminalSession.ActiveView.Viewer && session.openViewRequest != null) {
                            ViewerScreen(
                                data = session.openViewRequest,
                                onClose = {
                                    viewModel.switchToTerminal(session.id)
                                }
                            )
                        }
                    } else {
                        currentScreen = Screen.Main
                    }
                }
            }
        }
    }
}
