use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, State, Emitter};
use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};
use base64::prelude::*;
use std::net::SocketAddr;
use std::str::FromStr;
use ipnetwork::IpNetwork;

// Type alias for the gotatun device with default transports
type TunnelDevice = gotatun::device::Device<(
    gotatun::udp::socket::UdpSocketFactory,
    gotatun::tun::tun_async_device::TunDevice,
    gotatun::tun::tun_async_device::TunDevice,
)>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireguardConfig {
    pub private_key: String,
    pub peer_public_key: String,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub internal_ip: String,
}

pub struct NetworkManager {
    pub active_device: Arc<Mutex<Option<TunnelDevice>>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            active_device: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
pub async fn start_tunnel(app_handle: AppHandle, state: State<'_, NetworkManager>, config: WireguardConfig) -> Result<(), String> {
    let res = start_tunnel_inner(app_handle.clone(), &state, config).await;
    match res {
        Ok(_) => {
            app_handle.emit("tunnel-status", "connected").ok();
            Ok(())
        },
        Err(e) => {
            let err_msg = e.to_string();
            app_handle.emit("tunnel-error", &err_msg).ok();
            Err(err_msg)
        }
    }
}

async fn start_tunnel_inner(_app_handle: AppHandle, state: &NetworkManager, config: WireguardConfig) -> Result<()> {
    // 1. Decode keys
    let priv_key_bytes = BASE64_STANDARD.decode(&config.private_key)
        .map_err(|_| anyhow!("Invalid private key base64"))?;
    if priv_key_bytes.len() != 32 {
        return Err(anyhow!("Private key must be 32 bytes"));
    }
    let mut priv_key_arr = [0u8; 32];
    priv_key_arr.copy_from_slice(&priv_key_bytes);
    let priv_key = gotatun::x25519::StaticSecret::from(priv_key_arr);

    let pub_key_bytes = BASE64_STANDARD.decode(&config.peer_public_key)
        .map_err(|_| anyhow!("Invalid peer public key base64"))?;
    if pub_key_bytes.len() != 32 {
        return Err(anyhow!("Peer public key must be 32 bytes"));
    }
    let mut pub_key_arr = [0u8; 32];
    pub_key_arr.copy_from_slice(&pub_key_bytes);
    let peer_pub_key = gotatun::x25519::PublicKey::from(pub_key_arr);

    // 2. Parse endpoint
    let endpoint = SocketAddr::from_str(&config.endpoint)
        .map_err(|_| anyhow!("Invalid endpoint address"))?;

    // 3. Parse allowed IPs
    let mut allowed_ips = Vec::new();
    for ip_str in config.allowed_ips {
        let net = IpNetwork::from_str(&ip_str)
            .map_err(|_| anyhow!("Invalid allowed IP: {}", ip_str))?;
        allowed_ips.push(net);
    }

    // 4. Create Peer
    let mut peer = gotatun::device::Peer::new(peer_pub_key)
        .with_endpoint(endpoint)
        .with_allowed_ips(allowed_ips);
    peer.keepalive = Some(25);

    // 5. Build device
    // Note: Creating TUN device "zn0"
    let builder = gotatun::device::build()
        .with_default_udp()
        .create_tun("zn0")
        .map_err(|e| anyhow!("Failed to create TUN device: {}", e))?
        .with_peer(peer);
    
    let device = builder.build().await
        .map_err(|e| anyhow!("Failed to build device: {}", e))?;

    // 6. Set private key
    device.write(async |w| {
        w.set_private_key(priv_key).await;
    }).await.map_err(|e| anyhow!("Failed to set private key: {}", e))?;

    // 7. Store device
    let mut active = state.active_device.lock().await;
    *active = Some(device);

    Ok(())
}

#[tauri::command]
pub async fn stop_tunnel(app_handle: AppHandle, state: State<'_, NetworkManager>) -> Result<(), String> {
    let mut active = state.active_device.lock().await;
    if let Some(device) = active.take() {
        device.stop().await;
        app_handle.emit("tunnel-status", "disconnected").ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_manager_init() {
        let manager = NetworkManager::new();
        let status = manager.active_device.lock().await;
        assert!(status.is_none());
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