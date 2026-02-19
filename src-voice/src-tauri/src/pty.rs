use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tracing::{error, info};

pub struct PtyState {
    tx: mpsc::Sender<Vec<u8>>,
    master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
}

impl PtyState {
    pub fn new(
        tx: mpsc::Sender<Vec<u8>>,
        master: Box<dyn portable_pty::MasterPty + Send>,
    ) -> Self {
        Self {
            tx,
            master: Mutex::new(master),
        }
    }
}

#[tauri::command]
pub async fn pty_spawn(
    app_handle: AppHandle,
    pty_state: tauri::State<'_, Mutex<Option<PtyState>>>,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| {
            error!("Failed to open PTY: {}", e);
            e.to_string()
        })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let cmd = CommandBuilder::new(&shell);

    info!("Spawning shell: {}", shell);
    let _child = pair.slave.spawn_command(cmd).map_err(|e| {
        let err_msg = format!("Failed to spawn shell ({}): {}", shell, e);
        error!("{}", err_msg);
        err_msg
    })?;

    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);

    // Reader task: PTY output → frontend events
    let app_clone = app_handle.clone();
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    info!("PTY reader EOF");
                    break;
                }
                Ok(n) => {
                    let _ = app_clone.emit(
                        "pty-output",
                        serde_json::json!({
                            "data": String::from_utf8_lossy(&buf[..n])
                        }),
                    );
                }
                Err(e) => {
                    error!("PTY reader error: {}", e);
                    break;
                }
            }
        }
        let _ = app_clone.emit("pty-closed", ());
    });

    // Writer task: mpsc channel → PTY stdin
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if let Err(e) = writer.write_all(&data) {
                error!("PTY writer error: {}", e);
                break;
            }
        }
    });

    let state = PtyState::new(tx, pair.master);
    *pty_state.lock().map_err(|e| e.to_string())? = Some(state);

    info!("PTY shell spawned successfully");
    Ok(())
}

#[tauri::command]
pub async fn pty_write(
    pty_state: tauri::State<'_, Mutex<Option<PtyState>>>,
    data: Vec<u8>,
) -> Result<(), String> {
    // Extract the sender without holding the guard across await
    let tx = {
        let guard = pty_state.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(state) => state.tx.clone(),
            None => return Err("No active PTY session".to_string()),
        }
    };
    tx.send(data)
        .await
        .map_err(|_| "Failed to send to PTY".to_string())
}

#[tauri::command]
pub fn pty_resize(
    pty_state: tauri::State<'_, Mutex<Option<PtyState>>>,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let guard = pty_state.lock().map_err(|e| e.to_string())?;
    if let Some(state) = guard.as_ref() {
        state
            .master
            .lock()
            .map_err(|e| e.to_string())?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("No active PTY session".to_string())
    }
}

#[cfg(test)]
mod tests {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};

    #[test]
    fn test_pty_spawn_and_roundtrip() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let cmd = CommandBuilder::new("cat");
        let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn cat");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();

        writer.write_all(b"hello\n").unwrap();
        drop(writer);

        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();
        assert!(output.contains("hello"));
    }
}
