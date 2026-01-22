package com.zelland.ui

import android.annotation.SuppressLint
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import coil.compose.AsyncImage
import coil.request.ImageRequest
import com.zelland.model.TerminalSession

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ViewerScreen(
    data: TerminalSession.OpenViewData,
    onClose: () -> Unit
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(data.title, maxLines = 1) },
                actions = {
                    IconButton(onClick = onClose) {
                        Icon(Icons.Default.Close, contentDescription = "Close Viewer")
                    }
                }
            )
        }
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Color.Black),
            contentAlignment = Alignment.Center
        ) {
            when (data.fileType) {
                TerminalSession.FileType.IMAGE -> {
                    ZoomableImage(url = data.url)
                }
                TerminalSession.FileType.PDF, TerminalSession.FileType.MARKDOWN -> {
                    // For now, use WebView for PDF and Markdown
                    // We'll specialize Markdown in Milestone 13
                    GenericWebViewer(url = data.url)
                }
                else -> {
                    Text("Unsupported file type", color = Color.White)
                }
            }
        }
    }
}

@Composable
fun ZoomableImage(url: String) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    val state = rememberTransformableState { zoomChange, offsetChange, _ ->
        scale *= zoomChange
        offset += offsetChange
    }

    AsyncImage(
        model = ImageRequest.Builder(LocalContext.current)
            .data(url)
            .crossfade(true)
            .build(),
        contentDescription = null,
        modifier = Modifier
            .fillMaxSize()
            .graphicsLayer(
                scaleX = maxOf(.5f, minOf(5f, scale)),
                scaleY = maxOf(.5f, minOf(5f, scale)),
                translationX = offset.x,
                translationY = offset.y
            )
            .transformable(state = state),
        contentScale = ContentScale.Fit
    )
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
fun GenericWebViewer(url: String) {
    AndroidView(
        factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.builtInZoomControls = true
                settings.displayZoomControls = false
                webViewClient = WebViewClient()
                loadUrl(url)
            }
        },
        modifier = Modifier.fillMaxSize()
    )
}
