pub mod ssh;
pub mod daemon;
pub mod intent;

use tauri::{State, AppHandle};
use crate::ssh::{SshConfig, SshManager};
use crate::daemon::DaemonManager;

#[tauri::command]
async fn ssh_connect(state: State<'_, SshManager>, config: SshConfig) -> Result<(), String> {
    state.connect(config).await
}

#[tauri::command]
async fn ssh_disconnect(state: State<'_, SshManager>) -> Result<(), ()> {
    state.disconnect().await;
    Ok(())
}

#[tauri::command]
async fn ssh_exec(state: State<'_, SshManager>, command: String) -> Result<String, String> {
    state.execute_command(command).await
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
            ssh_exec,
            daemon_connect
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}