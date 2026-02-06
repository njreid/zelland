use tauri::{AppHandle, State, Emitter};
use crate::ssh::{SshConfig, SshManager};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;
use std::io::{Read, Write};

pub struct MoshSession {
    pub tx: mpsc::Sender<Vec<u8>>,
    pub _child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct MoshManager {
    pub active_sessions: Arc<Mutex<HashMap<String, MoshSession>>>,
}

impl MoshManager {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tauri::command]
pub async fn mosh_connect(
    app_handle: AppHandle,
    ssh_manager: State<'_, SshManager>,
    mosh_manager: State<'_, MoshManager>,
    tab_id: String,
    config: SshConfig,
) -> Result<(), String> {
    // 1. Start mosh-server on remote via SSH
    // We want to run mosh-server and get the connection string
    let cmd = "mosh-server new -c 256".to_string();
    let output = ssh_manager.run_command(config.clone(), cmd).await?;
    
    // 2. Parse output for MOSH CONNECT <PORT> <KEY>
    let connect_line = output.lines()
        .find(|line| line.contains("MOSH CONNECT"))
        .ok_or_else(|| format!("Mosh server did not return connection string. Output: {}", output))?;
    
    let parts: Vec<&str> = connect_line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(format!("Invalid MOSH CONNECT string: {}", connect_line));
    }
    let port = parts[2];
    let key = parts[3];

    // 3. Spawn mosh-client locally in a PTY
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }).map_err(|e| e.to_string())?;

    // On Linux, the mosh-client command is:
    // MOSH_KEY=<KEY> mosh-client <IP> <PORT>
    // We'll use the internal IP from WireGuard if available, or the one from config.
    // For now, use config.host.
    
    let mut cmd_builder = CommandBuilder::new("mosh-client");
    cmd_builder.arg(config.host);
    cmd_builder.arg(port);
    cmd_builder.env("MOSH_KEY", key);

    let child = pair.slave.spawn_command(cmd_builder).map_err(|e| e.to_string())?;
    
    // We need to move the slave out of the pair before we start reading/writing
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
    let tab_id_clone = tab_id.clone();
    let app_handle_clone = app_handle.clone();

    // Spawn reader task
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = app_handle_clone.emit("mosh-output", serde_json::json!({
                        "tabId": tab_id_clone,
                        "data": String::from_utf8_lossy(&buf[..n])
                    }));
                }
                Err(_) => break,
            }
        }
        let _ = app_handle_clone.emit("mosh-closed", tab_id_clone);
    });

    // Spawn writer task
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if let Err(_) = writer.write_all(&data) {
                break;
            }
        }
    });

    mosh_manager.active_sessions.lock().await.insert(tab_id, MoshSession {
        tx,
        _child: child,
    });

    Ok(())
}

#[tauri::command]
pub async fn mosh_write(
    state: State<'_, MoshManager>,
    tab_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let active = state.active_sessions.lock().await;
    if let Some(session) = active.get(&tab_id) {
        session.tx.send(data).await.map_err(|_| "Failed to send to mosh".to_string())?;
        Ok(())
    } else {
        Err("No active mosh session".to_string())
    }
}
