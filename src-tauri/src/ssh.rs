use serde::{Deserialize, Serialize};
use russh::*;
use russh::client::AuthResult;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    pub session: Arc<Mutex<Option<client::Handle<Client>>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(&self, config: SshConfig) -> Result<(), String> {
        let mut session_guard = self.session.lock().await;
        
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
                *session_guard = Some(session);
                Ok(())
            }
            Ok(_) => Err("Authentication failed".to_string()),
            Err(e) => Err(format!("Authentication error: {}", e)),
        }
    }

    pub async fn disconnect(&self) {
        let mut session_guard = self.session.lock().await;
        *session_guard = None;
    }

    pub async fn execute_command(&self, command: String) -> Result<String, String> {
        let mut session_guard = self.session.lock().await;
        let session = session_guard.as_mut().ok_or("Not connected")?;

        let mut channel = session.channel_open_session().await
            .map_err(|e| format!("Failed to open channel: {}", e))?;

        channel.exec(true, command).await
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let mut output = String::new();
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { ref data } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                russh::ChannelMsg::ExitStatus { .. } => break,
                russh::ChannelMsg::Eof => break,
                _ => {}
            }
        }

        Ok(output)
    }
}