use serde::{Deserialize, Serialize};
use russh::*;
use russh::client::AuthResult;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;
use std::future::Future;
use log::{info, error, debug};
use tauri::ipc::Channel;
use crate::keystore::KeyManager;
use crate::terminal::TerminalSession;

// ---------------------------------------------------------------------------
// SSH plumbing
// ---------------------------------------------------------------------------

/// Messages sent from the SSH session loop to the frontend via a Tauri Channel.
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SshChannelMsg {
    /// Full viewport rendered by Rust as ANSI escape sequences.
    Viewport { 
        #[serde(with = "serde_bytes")]
        data: Vec<u8>, 
        at_bottom: bool,
        mouse_mode: bool,
    },
    /// SSH session has ended (clean exit or connection drop).
    Closed,
}

/// Establish an authenticated SSH session, handling both connection and auth.
async fn open_session(
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>,
) -> Result<client::Handle<Client>, String> {
    let addr = format!("{}:{}", config.host, config.port);
    let mut session = client::connect(Arc::new(client::Config::default()), addr, Client {})
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    match authenticate(&mut session, config, key_manager).await? {
        AuthResult::Success => Ok(session),
        _ => Err("Authentication failed".to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AuthMethod {
    Password,
    PrivateKey,
    Key, // KeyStore managed key
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
    pub key_id: Option<String>,
    pub session_name: String,
}

pub enum SessionMsg {
    Data(Vec<u8>),
    Resize { rows: u32, cols: u32 },
    Mouse { x: f32, y: f32, action: String },
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

/// Load a private key from the given config or keystore.
async fn load_private_key(
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>
) -> Result<russh::keys::PrivateKey, String> {
    match config.auth_method {
        AuthMethod::PrivateKey => {
            let key_path = config.private_key_path.as_deref().ok_or("Private key path is required")?;
            let key_str = std::fs::read_to_string(key_path)
                .map_err(|e| format!("Failed to read private key at {}: {}", key_path, e))?;
            let passphrase = config.private_key_passphrase.as_deref();
            russh::keys::decode_secret_key(&key_str, passphrase)
                .map_err(|e| format!("Failed to decode private key: {}", e))
        }
        AuthMethod::Key => {
            let key_id = config.key_id.as_deref().ok_or("Key ID is required")?;
            key_manager.get_russh_key(key_id).await
        }
        AuthMethod::Password => {
            Err("load_private_key called with Password auth method".to_string())
        }
    }
}

/// Authenticate an SSH session using the given config and key manager.
async fn authenticate(
    session: &mut client::Handle<Client>,
    config: &SshConfig,
    key_manager: Arc<dyn KeyManager>,
) -> Result<AuthResult, String> {
    match config.auth_method {
        AuthMethod::Password => {
            let password = config.password.as_deref().ok_or("Password is required")?;
            session.authenticate_password(&config.username, password).await
                .map_err(|e| format!("Password auth error: {}", e))
        }
        AuthMethod::PrivateKey | AuthMethod::Key => {
            let key = load_private_key(config, key_manager).await?;
            session.authenticate_publickey(
                &config.username,
                russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None),
            ).await.map_err(|e| format!("Public key auth error: {}", e))
        }
    }
}

pub struct SshManager {
    // Stores the write handle for each active session
    pub active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<SessionMsg>>>>,
    pub focused_session: Arc<Mutex<Option<String>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            focused_session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn run_command(
        &self,
        config: SshConfig,
        cmd: String,
        key_manager: Arc<dyn KeyManager>,
    ) -> Result<String, String> {
        debug!("Running SSH command: {} on {}:{}", cmd, config.host, config.port);
        let session = open_session(&config, key_manager).await?;

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
                russh::ChannelMsg::Data { ref data } => output.extend_from_slice(data),
                russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Eof => break,
                _ => {}
            }
        }
        debug!("SSH command execution completed");
        Ok(String::from_utf8_lossy(&output).to_string())
    }

    pub async fn connect(
        &self,
        tab_id: String,
        config: SshConfig,
        rows: u16,
        cols: u16,
        output: Channel<SshChannelMsg>,
        key_manager: Arc<dyn KeyManager>,
    ) -> Result<(), String> {
        info!("SSH connect requested for tab: {}, host: {}", tab_id, config.host);
        let session = open_session(&config, key_manager).await
            .map_err(|e| {
                error!("SSH connect failed for tab {}: {}", tab_id, e);
                e
            })?;

        info!("SSH authentication successful for tab: {}", tab_id);
        let mut channel = session.channel_open_session().await
            .map_err(|e| {
                error!("Failed to open SSH channel for tab {}: {}", tab_id, e);
                format!("Failed to open channel: {}", e)
            })?;

        // Initialize PTY with provided dimensions
        channel.request_pty(true, "xterm-256color", cols as u32, rows as u32, 0, 0, &[]).await
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

        tokio::spawn(async move {
            info!("SSH session loop started for tab {}", tab_id_spawn);

            let mut ts = TerminalSession::new(cols, rows);
            
            // Optimized flush interval (60 FPS)
            let mut flush_interval = tokio::time::interval(std::time::Duration::from_millis(16));
            flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(SessionMsg::Data(data)) => {
                                if let Err(e) = channel.data(&data[..]).await {
                                    error!("Failed to write to SSH channel for tab {}: {}", tab_id_spawn, e);
                                    break;
                                }
                            }
                            Some(SessionMsg::Resize { rows, cols }) => {
                                if let Err(e) = channel.window_change(cols, rows, 0, 0).await {
                                    error!("Failed to resize SSH window for tab {}: {}", tab_id_spawn, e);
                                }
                                ts.resize(cols as u16, rows as u16);
                            }
                            Some(SessionMsg::Mouse { x, y, action }) => {
                                if let Some(encoded) = ts.encode_mouse_event(x, y, &action) {
                                    if let Err(e) = channel.data(&encoded[..]).await {
                                        error!("Failed to write mouse data to SSH channel for tab {}: {}", tab_id_spawn, e);
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { ref data }) => {
                                ts.process_bytes(data);
                            }
                            Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                info!("SSH channel exited with status {} for tab {}", exit_status, tab_id_spawn);
                                break;
                            }
                            Some(russh::ChannelMsg::Eof) => {
                                info!("SSH channel EOF for tab {}", tab_id_spawn);
                                break;
                            }
                            Some(_) => {}
                            None => break,
                        }
                    }
                    _ = flush_interval.tick() => {
                        if ts.is_dirty() {
                            // Phase 3: Trigger native wgpu rendering
                            ts.render_native();

                            // Strategy 1: Send only the visible viewport rendered by Rust.
                            // This eliminates double-parsing (Rust and JS both parsing ANSI).
                            let mouse_mode = ts.get_mouse_mode();
                            let viewport = ts.render_viewport();
                            let _ = output.send(SshChannelMsg::Viewport { 
                                data: viewport.to_vec(), 
                                at_bottom: true,
                                mouse_mode,
                            });
                        }
                    }
                }
            }

            let _ = output.send(SshChannelMsg::Closed);
        });

        self.active_sessions.lock().await.insert(tab_id.clone(), tx);
        {
            let mut focused = self.focused_session.lock().await;
            if focused.is_none() {
                *focused = Some(tab_id.clone());
            }
        }
        info!("SSH connection established for tab: {}", tab_id);
        Ok(())
    }

    /// Look up a session's sender, dropping the lock before the async send.
    pub async fn send_to_session(&self, tab_id: &str, msg: SessionMsg) -> Result<(), String> {
        let tx = {
            let active = self.active_sessions.lock().await;
            active.get(tab_id)
                .cloned()
                .ok_or_else(|| {
                    error!("No active SSH session for tab: {}", tab_id);
                    "No active session".to_string()
                })?
        };
        tx.send(msg).await
            .map_err(|e| format!("Failed to send to channel: {}", e))
    }

    pub async fn write_input(&self, tab_id: String, data: Vec<u8>) -> Result<(), String> {
        self.send_to_session(&tab_id, SessionMsg::Data(data)).await
    }

    pub async fn resize(&self, tab_id: String, rows: u32, cols: u32) -> Result<(), String> {
        self.send_to_session(&tab_id, SessionMsg::Resize { rows, cols }).await
    }

    pub async fn scroll(&self, _tab_id: String, _delta: i32) -> Result<(), String> {
        // Scrollback handled by Zellij only.
        Ok(())
    }

    pub async fn disconnect(&self, tab_id: String) {
        info!("Disconnecting SSH session for tab: {}", tab_id);
        let mut active = self.active_sessions.lock().await;
        active.remove(&tab_id);
    }

    pub async fn process_touch(&self, action: String, x: f32, y: f32) -> Result<(), String> {
        let tab_id = {
            let focused = self.focused_session.lock().await;
            focused.clone().ok_or("No focused session")?
        };
        self.send_to_session(&tab_id, SessionMsg::Mouse { x, y, action }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_method_serde_variants() {
        for (method, expected) in [
            (AuthMethod::Password, "\"Password\""),
            (AuthMethod::PrivateKey, "\"PrivateKey\""),
            (AuthMethod::Key, "\"Key\""),
        ] {
            let json = serde_json::to_string(&method).unwrap();
            assert_eq!(json, expected);
            let _: AuthMethod = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_ssh_channel_msg_serialization() {
        let msg = SshChannelMsg::Viewport { 
            data: vec![1, 2, 3], 
            at_bottom: true,
            mouse_mode: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // With tag="type" and content="data", it should be {"type":"Viewport","data":{"data":[1,2,3],"at_bottom":true,"mouse_mode":false}}
        assert!(json.contains("\"type\":\"Viewport\""));
        assert!(json.contains("\"data\":[1,2,3]"));
        assert!(json.contains("\"at_bottom\":true"));
        assert!(json.contains("\"mouse_mode\":false"));
    }
}
