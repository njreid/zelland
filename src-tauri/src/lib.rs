pub mod ssh;
pub mod daemon;
pub mod intent;

use tauri::{State, AppHandle};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SshManager::new())
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
            daemon_connect
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}