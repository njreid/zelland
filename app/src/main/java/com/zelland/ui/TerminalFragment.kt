package com.zelland.ui

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.ProgressBar
import android.widget.Toast
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import com.zelland.R
import com.zelland.viewmodel.TerminalViewModel

/**
 * Fragment that displays a Zellij web terminal in a WebView
 */
class TerminalFragment : Fragment() {

    private val viewModel: TerminalViewModel by activityViewModels()
    private lateinit var webView: WebView
    private lateinit var progressBar: ProgressBar
    private var sessionId: String? = null

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val view = inflater.inflate(R.layout.fragment_terminal, container, false)

        webView = view.findViewById(R.id.webView)
        progressBar = view.findViewById(R.id.progressBar)

        sessionId = arguments?.getString(ARG_SESSION_ID)

        setupWebView()
        loadSession()

        return view
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupWebView() {
        webView.settings.apply {
            // Enable JavaScript (required for Zellij web)
            javaScriptEnabled = true

            // Enable DOM storage (required for Zellij)
            domStorageEnabled = true

            // Enable database storage
            databaseEnabled = true

            // Allow file access
            allowFileAccess = true
            allowContentAccess = true

            // Enable zoom controls (but hide default buttons)
            setSupportZoom(true)
            builtInZoomControls = true
            displayZoomControls = false

            // Set user agent (in case Zellij checks for mobile)
            userAgentString = "Mozilla/5.0 (Linux; Android) AppleWebKit/537.36 Zelland/1.0"

            // Mixed content mode (allow HTTP in Tailscale network)
            @Suppress("DEPRECATION")
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW

            // Load with overview mode
            loadWithOverviewMode = true
            useWideViewPort = true
        }

        // WebViewClient for handling page loading
        webView.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(
                view: WebView?,
                request: WebResourceRequest?
            ): Boolean {
                // Let WebView handle all Zellij URLs
                return false
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                progressBar.visibility = View.GONE
                android.util.Log.d("TerminalFragment", "Page loaded: $url")
            }

            override fun onReceivedError(
                view: WebView?,
                request: WebResourceRequest?,
                error: WebResourceError?
            ) {
                super.onReceivedError(view, request, error)
                progressBar.visibility = View.GONE

                val errorMessage = "Failed to load Zellij web: ${error?.description}"
                android.util.Log.e("TerminalFragment", errorMessage)

                Toast.makeText(
                    requireContext(),
                    "Connection error. Check if Tailscale is connected.",
                    Toast.LENGTH_LONG
                ).show()
            }
        }

        // WebChromeClient for progress updates
        webView.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                super.onProgressChanged(view, newProgress)
                if (newProgress < 100) {
                    progressBar.visibility = View.VISIBLE
                    progressBar.progress = newProgress
                } else {
                    progressBar.visibility = View.GONE
                }
            }
        }
    }

    private fun loadSession() {
        val id = sessionId ?: return

        viewModel.sessions.observe(viewLifecycleOwner) { sessions ->
            val session = sessions.find { it.id == id }
            if (session != null && session.isConnected && session.localUrl != null) {
                android.util.Log.i("TerminalFragment", "Loading Zellij URL: ${session.localUrl}")
                webView.loadUrl(session.localUrl)
            } else if (session != null && !session.isConnected) {
                android.util.Log.w("TerminalFragment", "Session disconnected")
                Toast.makeText(
                    requireContext(),
                    "Session disconnected",
                    Toast.LENGTH_SHORT
                ).show()
            }
        }
    }

    override fun onPause() {
        super.onPause()
        // Pause WebView when fragment is paused
        webView.onPause()
    }

    override fun onResume() {
        super.onResume()
        // Resume WebView when fragment is resumed
        webView.onResume()
    }

    override fun onDestroyView() {
        super.onDestroyView()
        // Clean up WebView
        webView.destroy()
    }

    companion object {
        private const val ARG_SESSION_ID = "session_id"

        fun newInstance(sessionId: String): TerminalFragment {
            return TerminalFragment().apply {
                arguments = Bundle().apply {
                    putString(ARG_SESSION_ID, sessionId)
                }
            }
        }
    }
}
