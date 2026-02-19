pub mod pty;
pub mod speech;

use std::sync::{Arc, Mutex};

pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(window) = _app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .manage(Mutex::new(None::<pty::PtyState>))
        .manage(Arc::new(Mutex::new(speech::SpeechState::new())))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            speech::init_speech,
            speech::start_recording,
            speech::stop_and_transcribe,
            speech::list_audio_devices,
            speech::set_audio_device,
            speech::set_input_gain,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
