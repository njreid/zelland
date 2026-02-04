use serde::{Deserialize, Serialize};
use russh::*;
use russh::client::AuthResult;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;

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
}

pub struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;
}

pub struct SshManager {
    // Stores the write handle for each active session
    pub active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<Vec<u8>>>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn connect(&self, tab_id: String, config: SshConfig, app_handle: tauri::AppHandle) -> Result<(), String> {
        let client_config = client::Config {
            ..Default::default()
        };
        let client_config = Arc::new(client_config);
        let sh = Client {};

        let addr = format!("{}:{}", config.host, config.port);
        let mut session = client::connect(client_config, addr, sh).await
            .map_err(|e| format!("Connection failed: {}", e))?;

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
                let mut channel = session.channel_open_session().await
                    .map_err(|e| format!("Failed to open channel: {}", e))?;
                
                channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await
                    .map_err(|e| format!("Failed to request PTY: {}", e))?;
                
                channel.request_shell(true).await
                    .map_err(|e| format!("Failed to request shell: {}", e))?;

                let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
                let tab_id_clone = tab_id.clone();
                let app_handle_clone = app_handle.clone();

                tokio::spawn(async move {
                    use tauri::Emitter;
                    loop {
                        tokio::select! {
                            // Read from frontend, write to SSH
                            Some(data) = rx.recv() => {
                                if let Err(_) = channel.data(&data[..]).await {
                                    break;
                                }
                            }
                            // Read from SSH, emit to frontend
                            Some(msg) = channel.wait() => {
                                match msg {
                                    russh::ChannelMsg::Data { ref data } => {
                                        let _ = app_handle_clone.emit("ssh-output", serde_json::json!({
                                            "tabId": tab_id_clone,
                                            "data": String::from_utf8_lossy(data)
                                        }));
                                    }
                                    russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Eof => break,
                                    _ => {}
                                }
                            }
                            else => break
                        }
                    }
                    // Session ended
                    let _ = app_handle_clone.emit("ssh-closed", tab_id_clone);
                });

                self.active_sessions.lock().await.insert(tab_id, tx);
                Ok(())
            }
            Ok(_) => Err("Authentication failed".to_string()),
            Err(e) => Err(format!("Authentication error: {}", e)),
        }
    }

    pub async fn write_input(&self, tab_id: String, data: Vec<u8>) -> Result<(), String> {
        let active = self.active_sessions.lock().await;
        if let Some(tx) = active.get(&tab_id) {
            tx.send(data).await.map_err(|_| "Failed to send to channel".to_string())?;
            Ok(())
        } else {
            Err("No active session".to_string())
        }
    }

    pub async fn disconnect(&self, tab_id: String) {
        let mut active = self.active_sessions.lock().await;
        active.remove(&tab_id);
    }
}