pub mod ssh;
pub mod daemon;
pub mod intent;
pub mod network;
pub mod keystore;

use tauri::{State, AppHandle};
use crate::ssh::{SshConfig, SshManager};
use crate::daemon::DaemonManager;
use crate::keystore::{KeyManager, StandardKeyManager};

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
async fn ssh_resize(state: State<'_, SshManager>, tab_id: String, rows: u32, cols: u32) -> Result<(), String> {
    state.resize(tab_id, rows, cols).await
}

#[tauri::command]
async fn daemon_connect(app_handle: AppHandle, url: String) -> Result<(), String> {
    let manager = DaemonManager::new(app_handle);
    manager.connect(url).await
}

#[tauri::command]
async fn ssh_list_zellij_sessions(state: State<'_, SshManager>, config: SshConfig) -> Result<Vec<String>, String> {
    let output = state.run_command(config, "zellij list-sessions -n -q".to_string()).await?;
    let sessions = output.lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(sessions)
}

#[tauri::command]
async fn run_remote_command(state: State<'_, SshManager>, config: SshConfig, command: String) -> Result<String, String> {
    state.run_command(config, command).await
}

#[tauri::command]
async fn close_window(app_handle: AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
async fn generate_ssh_key(app_handle: AppHandle, label: String) -> Result<crate::keystore::KeyIdentity, String> {
    let manager = StandardKeyManager::new(&app_handle);
    manager.generate_key(label).await
}

#[tauri::command]
async fn list_ssh_keys(app_handle: AppHandle) -> Result<Vec<crate::keystore::KeyIdentity>, String> {
    let manager = StandardKeyManager::new(&app_handle);
    manager.list_identities().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .manage(SshManager::new())
        .manage(network::NetworkManager::new())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_haptics::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(intent::init())
        .invoke_handler(tauri::generate_handler![
            ssh_connect,
            ssh_disconnect,
            ssh_write,
            ssh_resize,
            run_remote_command,
            daemon_connect,
            daemon::daemon_get_projects,
            daemon::daemon_activate_project,
            daemon::daemon_read_file,
            ssh_list_zellij_sessions,
            network::start_tunnel,
            network::stop_tunnel,
            generate_ssh_key,
            list_ssh_keys,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}