use tauri::{AppHandle, State, Emitter, Manager};
use crate::ssh::{SshConfig, SshManager};
use crate::ManagedKeyManager;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;
use std::io::{Read, Write};
use tauri_plugin_shell::ShellExt;
use std::process::Command as StdCommand;
use log::{info, error, debug, warn};

pub struct MoshSession {
    pub tx: mpsc::Sender<Vec<u8>>,
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub _child: Box<dyn portable_pty::Child + Send>,
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
    key_manager: State<'_, ManagedKeyManager>,
    mosh_manager: State<'_, MoshManager>,
    tab_id: String,
    config: SshConfig,
) -> Result<(), String> {
    info!("Mosh connection requested for tab: {}, host: {}", tab_id, config.host);
    // 1. Start mosh-server on remote via SSH.
    let cmd = format!("mosh-server new -c 256 -- sh -c \"zellij attach --create {} || $SHELL\"", config.session_name);
    debug!("Executing remote command via SSH: {}", cmd);
    let output = ssh_manager.run_command(app_handle.clone(), config.clone(), cmd, key_manager.0.clone()).await
        .map_err(|e| {
            error!("Failed to start mosh-server via SSH: {}", e);
            e
        })?;

    debug!("Mosh server output: {}", output);
    // 2. Parse output for MOSH CONNECT <PORT> <KEY>
    let connect_line = output.lines()
        .find(|line| line.contains("MOSH CONNECT"))
        .ok_or_else(|| {
            let err = format!("Mosh server did not return connection string. Output: {}", output);
            error!("{}", err);
            err
        })?;

    let parts: Vec<&str> = connect_line.split_whitespace().collect();
    if parts.len() < 4 {
        let err = format!("Invalid MOSH CONNECT string: {}", connect_line);
        error!("{}", err);
        return Err(err);
    }
    let port = parts[2];
    let key = parts[3];

    info!("Mosh server started on port: {}", port);

    // 3. Resolve host to IP
    let resolved_host = if config.host == "localhost" {
        "127.0.0.1".to_string()
    } else {
        match tokio::net::lookup_host(format!("{}:{}", config.host, port)).await {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    let ip = addr.ip().to_string();
                    debug!("Resolved {} to {}", config.host, ip);
                    ip
                } else {
                    debug!("Could not resolve {}, using as is", config.host);
                    config.host.clone()
                }
            }
            Err(e) => {
                debug!("Lookup failed for {}: {}, using as is", config.host, e);
                config.host.clone()
            }
        }
    };

    // 4. Resolve the mosh-client sidecar path
    debug!("Resolving mosh-client sidecar");

    #[cfg(target_os = "android")]
    let binary_path = {
        let triple = if cfg!(target_arch = "aarch64") {
            "aarch64-linux-android"
        } else if cfg!(target_arch = "arm") {
            "armv7-linux-androideabi"
        } else if cfg!(target_arch = "x86_64") {
            "x86_64-linux-android"
        } else if cfg!(target_arch = "x86") {
            "i686-linux-android"
        } else {
            return Err("Unsupported architecture".to_string());
        };
        let sidecar_name = format!("mosh-client-{}", triple);

        let files_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
        let cache_dir = app_handle.path().app_cache_dir().map_err(|e| e.to_string())?;

        let paths_to_check = [
            files_dir.join("tauri-shell-sidecar").join(&sidecar_name),
            files_dir.parent().unwrap().join("tauri-shell-sidecar").join(&sidecar_name),
            files_dir.parent().unwrap().join("no_backup").join("tauri-shell-sidecar").join(&sidecar_name),
            cache_dir.join("tauri-shell-sidecar").join(&sidecar_name),
        ];

        let mut resolved_path = None;
        for path in &paths_to_check {
            debug!("Checking sidecar path: {:?}", path);
            if path.exists() {
                debug!("Found sidecar at: {:?}", path);
                resolved_path = Some(path.clone());
                break;
            }
        }

        if resolved_path.is_none() {
            warn!("Sidecar not found in common locations. Listing directory contents for debugging...");
            if let Some(parent) = files_dir.parent() {
                debug!("Contents of {:?}:", parent);
                if let Ok(entries) = std::fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        debug!("  {:?}", entry.path());
                        if entry.path().is_dir() {
                            if let Ok(subentries) = std::fs::read_dir(entry.path()) {
                                for sub in subentries.flatten() {
                                    debug!("    {:?}", sub.path());
                                }
                            }
                        }
                    }
                }
            }
        }

        resolved_path.unwrap_or_else(|| {
            warn!("Falling back to plugin resolution");
            let sidecar_cmd = app_handle.shell().sidecar("mosh-client").unwrap();
            let std_cmd: StdCommand = sidecar_cmd.into();
            std_cmd.get_program().into()
        })
    };

    #[cfg(not(target_os = "android"))]
    let binary_path = {
        let sidecar_cmd = app_handle.shell().sidecar("mosh-client")
            .map_err(|e| format!("Failed to find mosh-client sidecar: {}", e))?;
        let std_cmd: StdCommand = sidecar_cmd.into();
        std_cmd.get_program().to_owned()
    };

    info!("Mosh client binary path determined: {:?}", binary_path);

    // 5. Spawn mosh-client locally in a PTY using the sidecar binary
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }).map_err(|e| {
        error!("Failed to open PTY: {}", e);
        e.to_string()
    })?;

    let mut cmd_builder = CommandBuilder::new(&binary_path);
    cmd_builder.arg(&resolved_host);
    cmd_builder.arg(port);
    cmd_builder.env("MOSH_KEY", key);

    info!("Spawning mosh-client: {:?} {} {}", binary_path, resolved_host, port);
    let child = pair.slave.spawn_command(cmd_builder).map_err(|e| {
        let err_msg = format!("Failed to spawn mosh-client ({:?}): {}", binary_path, e);
        error!("{}", err_msg);
        err_msg
    })?;

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
                Ok(0) => {
                    info!("Mosh reader EOF for tab: {}", tab_id_clone);
                    break;
                }
                Ok(n) => {
                    let _ = app_handle_clone.emit("mosh-output", serde_json::json!({
                        "tabId": tab_id_clone,
                        "data": String::from_utf8_lossy(&buf[..n])
                    }));
                }
                Err(e) => {
                    error!("Mosh reader error for tab {}: {}", tab_id_clone, e);
                    break;
                }
            }
        }
        let _ = app_handle_clone.emit("mosh-closed", tab_id_clone);
    });

    // Spawn writer task
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if let Err(e) = writer.write_all(&data) {
                error!("Mosh writer error: {}", e);
                break;
            }
        }
    });

    mosh_manager.active_sessions.lock().await.insert(tab_id.clone(), MoshSession {
        tx,
        master: pair.master,
        _child: child,
    });

    info!("Mosh connection established for tab: {}", tab_id);
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

#[tauri::command]
pub async fn mosh_resize(
    state: State<'_, MoshManager>,
    tab_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let active = state.active_sessions.lock().await;
    if let Some(session) = active.get(&tab_id) {
        session.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("No active mosh session".to_string())
    }
}
