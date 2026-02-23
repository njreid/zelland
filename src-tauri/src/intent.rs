use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime, AppHandle, Emitter,
};
use serde_json::Value;

#[tauri::command]
pub fn handle_notification_action<R: Runtime>(app: AppHandle<R>, payload: String) {
    if let Ok(nav) = serde_json::from_str::<Value>(&payload) {
        let _ = app.emit("navigate-to-session", nav);
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new("intent")
        .invoke_handler(tauri::generate_handler![handle_notification_action])
        .setup(|_app, _api| {
            Ok(())
        })
        .build()
}
