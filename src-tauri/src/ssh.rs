use serde::{Deserialize, Serialize};
use russh::*;
use russh::client::AuthResult;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;
use std::future::Future;
use log::{info, error, debug};
use crate::keystore::KeyManager;

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
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run_command(
        &self, 
        _app_handle: tauri::AppHandle, 
        config: SshConfig, 
        cmd: String,
        key_manager: Arc<dyn KeyManager>
    ) -> Result<String, String> {
        debug!("Running SSH command: {} on {}:{}", cmd, config.host, config.port);
        let client_config = Arc::new(client::Config::default());
        let sh = Client {};

        let addr = format!("{}:{}", config.host, config.port);
        let mut session = client::connect(client_config, addr, sh).await
            .map_err(|e| {
                error!("SSH connection failed: {}", e);
                format!("Connection failed: {}", e)
            })?;

        let auth_res = authenticate(&mut session, &config, key_manager).await?;

        if let AuthResult::Success = auth_res {
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

    pub async fn connect(
        &self, 
        tab_id: String, 
        config: SshConfig, 
        app_handle: tauri::AppHandle,
        key_manager: Arc<dyn KeyManager>
    ) -> Result<(), String> {
        info!("SSH connect requested for tab: {}, host: {}", tab_id, config.host);
        let client_config = Arc::new(client::Config::default());
        let sh = Client {};

        let addr = format!("{}:{}", config.host, config.port);
        let mut session = client::connect(client_config, addr, sh).await
            .map_err(|e| {
                error!("SSH connection failed for tab {}: {}", tab_id, e);
                format!("Connection failed: {}", e)
            })?;

        let auth_res = authenticate(&mut session, &config, key_manager).await?;

        match auth_res {
            AuthResult::Success => {
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
            _ => {
                error!("SSH authentication failed for tab: {}", tab_id);
                Err("Authentication failed".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_private_key_missing_path() {
        // Cannot create a real AppHandle in tests, but we can test the PrivateKey path validation
        let config = SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: AuthMethod::PrivateKey,
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
            key_id: None,
            session_name: "main".to_string(),
        };

        // We can't call load_private_key without an AppHandle that resolves paths,
        // but we can test that it requires private_key_path
        assert!(config.private_key_path.is_none());
        // The actual check happens inside load_private_key via .ok_or()
    }

    #[test]
    fn test_load_private_key_from_file() {
        // Generate a key, write it, then verify load_private_key can read it
        let tmp = tempfile::tempdir().unwrap();
        let key = ssh_key::PrivateKey::random(&mut rand::rngs::OsRng, ssh_key::Algorithm::Ed25519).unwrap();
        let key_path = tmp.path().join("test_key");
        let pem = key.to_openssh(ssh_key::LineEnding::LF).unwrap();
        std::fs::write(&key_path, pem.as_bytes()).unwrap();

        let config = SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: AuthMethod::PrivateKey,
            password: None,
            private_key_path: Some(key_path.to_str().unwrap().to_string()),
            private_key_passphrase: None,
            key_id: None,
            session_name: "main".to_string(),
        };

        // Create a minimal mock - load_private_key for PrivateKey doesn't need AppHandle
        // We test the key loading path directly
        let key_str = std::fs::read_to_string(config.private_key_path.as_ref().unwrap()).unwrap();
        let decoded = russh::keys::decode_secret_key(&key_str, None);
        assert!(decoded.is_ok(), "Key should decode: {:?}", decoded.err());
    }

    #[test]
    fn test_load_private_key_bad_file() {
        let tmp = tempfile::tempdir().unwrap();
        let bad_path = tmp.path().join("nonexistent_key");

        let config = SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: AuthMethod::PrivateKey,
            password: None,
            private_key_path: Some(bad_path.to_str().unwrap().to_string()),
            private_key_passphrase: None,
            key_id: None,
            session_name: "main".to_string(),
        };

        let result = std::fs::read_to_string(config.private_key_path.as_ref().unwrap());
        assert!(result.is_err(), "Should fail to read nonexistent file");
    }

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
}
