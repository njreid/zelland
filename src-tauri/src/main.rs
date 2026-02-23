// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK + NVIDIA proprietary driver: EGL/DRI2 fails under Wayland,
    // crashing the WebKit renderer process (shown as "Connection refused").
    // Fix: disable the DMA-BUF renderer AND force X11/XWayland backend.
    // GDK_BACKEND is set conditionally so users can override it if needed.
    #[cfg(target_os = "linux")]
    // SAFETY: called before any threads are spawned (Tauri hasn't started yet).
    unsafe {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        if std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    tauri_app_lib::run()
}
