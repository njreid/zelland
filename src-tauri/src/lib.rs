pub mod ssh;
pub mod daemon;
pub mod intent;
pub mod network;

use tauri::{State, AppHandle, Manager};
use crate::ssh::{SshConfig, SshManager};
use crate::daemon::DaemonManager;

#[tauri::command]
async fn ssh_connect(app_handle: AppHandle, state: State<'_, SshManager>, tab_id: String, config: SshConfig) -> Result<(), String> {
    state.connect(tab_id, config, app_handle).await
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
async fn daemon_connect(app_handle: AppHandle, url: String) -> Result<(), String> {
    let manager = DaemonManager::new(app_handle);
    manager.connect(url).await
}

#[tauri::command]
async fn ssh_list_zellij_sessions(state: State<'_, SshManager>, config: SshConfig) -> Result<Vec<String>, String> {
    // Run zellij list-sessions to get active sessions
    let output = state.run_command(config, "zellij list-sessions -n -q".to_string()).await?;
    let sessions = output.lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(sessions)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .manage(SshManager::new())
        .manage(network::NetworkManager::new())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_haptics::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(intent::init())
        .invoke_handler(tauri::generate_handler![
            ssh_connect,
            ssh_disconnect,
            ssh_write,
            daemon_connect,
            ssh_list_zellij_sessions,
            network::start_tunnel,
            network::stop_tunnel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
