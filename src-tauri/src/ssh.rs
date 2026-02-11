use serde::{Deserialize, Serialize};
use russh::*;
use russh::client::AuthResult;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;
use std::future::Future;
use log::{info, error, debug};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AuthMethod {
    Password,
    PrivateKey,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub session_name: String,
}

pub enum SessionMsg {
    Data(Vec<u8>),
    Resize { rows: u32, cols: u32 },
}

pub struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(true) }
    }
}

pub struct SshManager {
    // Stores the write handle for each active session
    pub active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<SessionMsg>>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run_command(&self, config: SshConfig, cmd: String) -> Result<String, String> {
        debug!("Running SSH command: {} on {}:{}", cmd, config.host, config.port);
        let client_config = client::Config {
            ..Default::default()
        };
        let client_config = Arc::new(client_config);
        let sh = Client {};

        let addr = format!("{}:{}", config.host, config.port);
        let mut session = client::connect(client_config, addr, sh).await
            .map_err(|e| {
                error!("SSH connection failed: {}", e);
                format!("Connection failed: {}", e)
            })?;

        let auth_res = match config.auth_method {
            AuthMethod::Password => {
                let password = config.password.clone().ok_or("Password is required")?;
                session.authenticate_password(&config.username, &password).await
            }
            AuthMethod::PrivateKey => {
                return Err("Private key auth not implemented yet".to_string());
            }
        };

        if let Ok(AuthResult::Success) = auth_res {
            debug!("SSH authentication successful for command execution");
            let mut channel = session.channel_open_session().await
                .map_err(|e| {
                    error!("Failed to open SSH channel: {}", e);
                    format!("Failed to open channel: {}", e)
                })?;
            
            channel.exec(true, cmd).await
                .map_err(|e| {
                    error!("Failed to execute SSH command: {}", e);
                    format!("Failed to execute: {}", e)
                })?;

            let mut output = Vec::new();
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => {
                        output.extend_from_slice(data);
                    }
                    russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Eof => break,
                    _ => {}
                }
            }
            debug!("SSH command execution completed");
            Ok(String::from_utf8_lossy(&output).to_string())
        } else {
            error!("SSH authentication failed for command execution");
            Err("Authentication failed".to_string())
        }
    }

    pub async fn connect(&self, tab_id: String, config: SshConfig, app_handle: tauri::AppHandle) -> Result<(), String> {
        info!("SSH connect requested for tab: {}, host: {}", tab_id, config.host);
        let client_config = client::Config {
            ..Default::default()
        };
        let client_config = Arc::new(client_config);
        let sh = Client {};

        let addr = format!("{}:{}", config.host, config.port);
        let mut session = client::connect(client_config, addr, sh).await
            .map_err(|e| {
                error!("SSH connection failed for tab {}: {}", tab_id, e);
                format!("Connection failed: {}", e)
            })?;

        let auth_res = match config.auth_method {
            AuthMethod::Password => {
                let password = config.password.clone().ok_or("Password is required")?;
                session.authenticate_password(&config.username, &password).await
            }
            AuthMethod::PrivateKey => {
                return Err("Private key auth not implemented yet".to_string());
            }
        };

        match auth_res {
            Ok(AuthResult::Success) => {
                info!("SSH authentication successful for tab: {}", tab_id);
                let mut channel = session.channel_open_session().await
                    .map_err(|e| {
                        error!("Failed to open SSH channel for tab {}: {}", tab_id, e);
                        format!("Failed to open channel: {}", e)
                    })?;
                
                channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await
                    .map_err(|e| {
                        error!("Failed to request PTY for tab {}: {}", tab_id, e);
                        format!("Failed to request PTY: {}", e)
                    })?;
                
                let robust_cmd = format!("zellij attach --create {} || $SHELL", config.session_name);
                debug!("Executing SSH shell command: {}", robust_cmd);
                
                channel.exec(true, robust_cmd).await
                    .map_err(|e| {
                        error!("Failed to execute shell command for tab {}: {}", tab_id, e);
                        format!("Failed to execute command: {}", e)
                    })?;

                let (tx, mut rx) = mpsc::channel::<SessionMsg>(100);
                let tab_id_spawn = tab_id.clone();
                let app_handle_clone = app_handle.clone();

                tokio::spawn(async move {
                    use tauri::Emitter;
                    info!("SSH session loop started for tab {}", tab_id_spawn);
                    loop {
                        tokio::select! {
                            Some(msg) = rx.recv() => {
                                match msg {
                                    SessionMsg::Data(data) => {
                                        if let Err(e) = channel.data(&data[..]).await {
                                            error!("Failed to write to SSH channel for tab {}: {}", tab_id_spawn, e);
                                            break;
                                        }
                                    }
                                    SessionMsg::Resize { rows, cols } => {
                                        if let Err(e) = channel.window_change(cols, rows, 0, 0).await {
                                            error!("Failed to resize SSH window for tab {}: {}", tab_id_spawn, e);
                                        }
                                    }
                                }
                            }
                            Some(msg) = channel.wait() => {
                                match msg {
                                    russh::ChannelMsg::Data { ref data } => {
                                        let _ = app_handle_clone.emit("ssh-output", serde_json::json!({
                                            "tabId": tab_id_spawn,
                                            "data": String::from_utf8_lossy(data)
                                        }));
                                    }
                                    russh::ChannelMsg::ExitStatus { exit_status } => {
                                        info!("SSH channel exited with status {} for tab {}", exit_status, tab_id_spawn);
                                        break;
                                    }
                                    russh::ChannelMsg::Eof => {
                                        info!("SSH channel EOF for tab {}", tab_id_spawn);
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            else => {
                                break;
                            }
                        }
                    }
                    let _ = app_handle_clone.emit("ssh-closed", tab_id_spawn);
                });

                self.active_sessions.lock().await.insert(tab_id.clone(), tx);
                info!("SSH connection established for tab: {}", tab_id);
                Ok(())
            }
            Ok(_) => {
                error!("SSH authentication failed for tab: {}", tab_id);
                Err("Authentication failed".to_string())
            },
            Err(e) => {
                error!("SSH authentication error for tab {}: {}", tab_id, e);
                Err(format!("Authentication error: {}", e))
            },
        }
    }

    pub async fn write_input(&self, tab_id: String, data: Vec<u8>) -> Result<(), String> {
        let active = self.active_sessions.lock().await;
        if let Some(tx) = active.get(&tab_id) {
            tx.send(SessionMsg::Data(data)).await.map_err(|e| {
                error!("Failed to send data to SSH channel for tab {}: {}", tab_id, e);
                "Failed to send to channel".to_string()
            })?;
            Ok(())
        } else {
            error!("No active SSH session for tab: {}", tab_id);
            Err("No active session".to_string())
        }
    }

    pub async fn resize(&self, tab_id: String, rows: u32, cols: u32) -> Result<(), String> {
        let active = self.active_sessions.lock().await;
        if let Some(tx) = active.get(&tab_id) {
            tx.send(SessionMsg::Resize { rows, cols }).await.map_err(|e| {
                error!("Failed to send resize to SSH channel for tab {}: {}", tab_id, e);
                "Failed to send resize".to_string()
            })?;
            Ok(())
        } else {
            error!("No active SSH session for tab: {}", tab_id);
            Err("No active session".to_string())
        }
    }

    pub async fn disconnect(&self, tab_id: String) {
        info!("Disconnecting SSH session for tab: {}", tab_id);
        let mut active = self.active_sessions.lock().await;
        active.remove(&tab_id);
    }
}