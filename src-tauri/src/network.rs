use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, State, Emitter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireguardConfig {
    pub private_key: String,
    pub peer_public_key: String,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub internal_ip: String,
}

pub struct NetworkManager {
    // We will hold the tunnel state here
    pub active_tunnel: Arc<Mutex<Option<bool>>>, // Placeholder for real state
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            active_tunnel: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
pub async fn start_tunnel(app_handle: AppHandle, state: State<'_, NetworkManager>, config: WireguardConfig) -> Result<(), String> {
    // TODO: Implement actual gotatun initialization
    println!("Starting tunnel with config: {:?}", config);
    let mut tunnel = state.active_tunnel.lock().await;
    *tunnel = Some(true);
    
    // Emit event to frontend
    app_handle.emit("tunnel-status", "connected").map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn stop_tunnel(app_handle: AppHandle, state: State<'_, NetworkManager>) -> Result<(), String> {
    let mut tunnel = state.active_tunnel.lock().await;
    *tunnel = Some(false);
    
    app_handle.emit("tunnel-status", "disconnected").map_err(|e| e.to_string())?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_manager_init() {
        let manager = NetworkManager::new();
        let status = manager.active_tunnel.lock().await;
        assert_eq!(*status, None);
    }

    #[test]
    fn test_config_serialization() {
        let config = WireguardConfig {
            private_key: "priv".to_string(),
            peer_public_key: "pub".to_string(),
            endpoint: "1.2.3.4:51820".to_string(),
            allowed_ips: vec!["10.0.0.0/24".to_string()],
            internal_ip: "10.0.0.2".to_string(),
        };
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: WireguardConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config.private_key, deserialized.private_key);
    }
}
