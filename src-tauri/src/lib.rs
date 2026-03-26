pub mod ssh;
pub mod terminal;
pub mod ghostty;
pub mod renderer;
pub mod daemon;
pub mod intent;
pub mod keybar;
pub mod network;
pub mod keystore;
#[cfg(target_os = "android")]
pub mod android_notif;

use tauri::{State, AppHandle, Manager};
use tauri::ipc::Channel;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use log::info;

static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));
pub fn get_app_handle() -> Option<AppHandle> {
    APP_HANDLE.lock().unwrap().clone()
}

/// Spawn a future on Tauri's async runtime. Safe to call from JNI/UI threads
/// that have no Tokio runtime context of their own.
pub fn spawn_on_runtime<F>(future: F) -> tauri::async_runtime::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    tauri::async_runtime::spawn(future)
}
#[cfg(target_os = "linux")]
use webkit2gtk::{SettingsExt, WebInspectorExt, WebViewExt};

use crate::ssh::{SshConfig, SshManager, SshChannelMsg};
use crate::daemon::DaemonManager;
use crate::keystore::KeyManager;
#[cfg(not(target_os = "android"))]
use crate::keystore::StandardKeyManager;
use std::sync::Arc;

pub struct ManagedKeyManager(pub Arc<dyn KeyManager>);

#[tauri::command]
async fn ssh_connect(
    ssh_state: State<'_, SshManager>,
    key_state: State<'_, ManagedKeyManager>,
    tab_id: String,
    config: SshConfig,
    rows: u16,
    cols: u16,
    channel: Channel<SshChannelMsg>,
) -> Result<(), String> {
    info!("ssh_connect called: tab={} host={}:{} user={} auth={:?} rows={} cols={}",
        tab_id, config.host, config.port, config.username, config.auth_method, rows, cols);
    let result = ssh_state.connect(tab_id.clone(), config, rows, cols, channel, key_state.0.clone()).await;
    match &result {
        Ok(_)    => info!("ssh_connect success: tab={}", tab_id),
        Err(e)   => info!("ssh_connect FAILED: tab={} err={}", tab_id, e),
    }
    result
}

#[tauri::command]
async fn ssh_disconnect(state: State<'_, SshManager>, tab_id: String) -> Result<(), ()> {
    state.disconnect(tab_id).await;
    Ok(())
}

#[tauri::command]
async fn ssh_write(state: State<'_, SshManager>, tab_id: String, data: Vec<u8>) -> Result<(), String> {
    state.write_input(tab_id, data).await
}

#[tauri::command]
async fn ssh_resize(state: State<'_, SshManager>, tab_id: String, rows: u32, cols: u32) -> Result<(), String> {
    state.resize(tab_id, rows, cols).await
}

#[tauri::command]
async fn ssh_scroll(state: State<'_, SshManager>, tab_id: String, delta: i32) -> Result<(), String> {
    state.scroll(tab_id, delta).await
}

#[tauri::command]
async fn daemon_connect(app_handle: AppHandle, state: State<'_, DaemonManager>, url: String) -> Result<(), String> {
    state.connect(url, app_handle).await
}

#[tauri::command]
async fn daemon_run_zellij_action(state: State<'_, DaemonManager>, action: String, session_name: String) -> Result<(), String> {
    state.send_action(action, session_name).await
}

#[tauri::command]
async fn ssh_list_zellij_sessions(
    ssh_state: State<'_, SshManager>,
    key_state: State<'_, ManagedKeyManager>,
    config: SshConfig,
) -> Result<Vec<String>, String> {
    let output = ssh_state.run_command(config, "zellij list-sessions -n -q".to_string(), key_state.0.clone()).await?;
    let sessions = output.lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(sessions)
}

#[tauri::command]
async fn run_remote_command(
    ssh_state: State<'_, SshManager>,
    key_state: State<'_, ManagedKeyManager>,
    config: SshConfig,
    command: String,
) -> Result<String, String> {
    ssh_state.run_command(config, command, key_state.0.clone()).await
}

#[tauri::command]
async fn show_agent_notification(title: String, body: String, session_name: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    crate::android_notif::show(&title, &body, &session_name);
    Ok(())
}

#[tauri::command]
async fn set_terminal_font_size(css_px: f32, dpr: f32) -> Result<(), String> {
    crate::renderer::update_font_size_global(css_px, dpr);
    Ok(())
}

#[tauri::command]
async fn close_window(app_handle: AppHandle) {
    app_handle.exit(0);
}

/// Called by the frontend to deliver the result of a biometric authentication prompt.
#[tauri::command]
async fn biometric_result(response: crate::keystore::BiometricResponse) {
    crate::keystore::complete_biometric_request(response);
}

#[tauri::command]
async fn generate_ssh_key(key_state: State<'_, ManagedKeyManager>, label: String) -> Result<crate::keystore::KeyIdentity, String> {
    key_state.0.generate_key(label).await
}

#[tauri::command]
async fn list_ssh_keys(key_state: State<'_, ManagedKeyManager>) -> Result<Vec<crate::keystore::KeyIdentity>, String> {
    key_state.0.list_identities().await
}

#[tauri::command]
async fn delete_ssh_key(key_state: State<'_, ManagedKeyManager>, id: String) -> Result<(), String> {
    key_state.0.delete_identity(id).await
}

#[cfg(target_os = "linux")]
fn setup_linux_devtools(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| {
        let webview = webview.inner();
        if let Some(settings) = WebViewExt::settings(&webview) {
            settings.set_enable_developer_extras(true);
        }
        if let Some(inspector) = webview.inspector() {
            println!("Linux: Inspector found, setting up detached hooks...");
            
            inspector.connect_attach(|inspector| {
                println!("Linux: Inspector 'attach' signal - detaching and inhibiting...");
                inspector.detach();
                true
            });

            inspector.connect_open_window(|_| {
                println!("Linux: Inspector 'open-window' signal...");
                false
            });
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            *APP_HANDLE.lock().unwrap() = Some(_app.handle().clone());

            #[cfg(debug_assertions)]
            if let Some(window) = _app.get_webview_window("main") {
                #[cfg(target_os = "linux")]
                setup_linux_devtools(&window);

                window.open_devtools();
            }

            #[cfg(target_os = "android")]
            let key_manager: Arc<dyn KeyManager> = Arc::new(crate::keystore::AndroidKeyManager::new(&_app.handle()));
            #[cfg(not(target_os = "android"))]
            let key_manager: Arc<dyn KeyManager> = Arc::new(StandardKeyManager::new(&_app.handle()));
            
            _app.manage(ManagedKeyManager(key_manager));

            Ok(())
        })
        .manage(SshManager::new())
        .manage(network::NetworkManager::new())
        .manage(DaemonManager::new())
        .plugin(tauri_plugin_log::Builder::new()
            .level(log::LevelFilter::Info) // Default to Info for app
            .level_for("russh", log::LevelFilter::Warn)
            .level_for("tokio_tungstenite", log::LevelFilter::Warn)
            .level_for("tungstenite", log::LevelFilter::Warn)
            .build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_haptics::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(intent::init())
        .plugin(keybar::init())
        .invoke_handler(tauri::generate_handler![
            ssh_connect,
            ssh_disconnect,
            ssh_write,
            ssh_resize,
            ssh_scroll,
            run_remote_command,
            daemon_connect,
            daemon_run_zellij_action,
            daemon::daemon_get_projects,
            daemon::daemon_activate_project,
            daemon::daemon_list_project_files,
            daemon::daemon_read_file,
            daemon::daemon_annotate_file,
            daemon::daemon_get_recent_sessions,
            daemon::daemon_record_session,
            ssh_list_zellij_sessions,
            network::start_tunnel,
            network::stop_tunnel,
            generate_ssh_key,
            list_ssh_keys,
            delete_ssh_key,
            close_window,
            biometric_result,
            set_terminal_font_size,
            show_agent_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
        }

        #[cfg(test)]
        mod tests {
        #[test]
        fn test_android_connectivity() {
        // This test will run on the Android device via Dinghy
        let os = std::env::consts::OS;
        println!("Running on OS: {}", os);
        assert!(true);
        }
        }