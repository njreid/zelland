use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("intent")
        .setup(|_app, _api| {
            Ok(())
        })
        .build()
}
