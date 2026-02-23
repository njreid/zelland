use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::assets::AssetManager;
use crate::proto::zelland::{self, envelope::Payload, Envelope, KeepAlive};

/// Broadcast channel capacity for WebSocket client fan-out.
const BROADCAST_CAPACITY: usize = 256;

/// Client registry backed by a tokio broadcast channel.
/// All connected clients receive every broadcasted message.
#[derive(Clone)]
pub struct ClientRegistry {
    tx: broadcast::Sender<Vec<u8>>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { tx }
    }

    /// Broadcast a protobuf Envelope to all connected clients.
    pub fn broadcast(&self, envelope: &Envelope) {
        let data = envelope.encode_to_vec();
        // Ignore error when no receivers are connected
        let _ = self.tx.send(data);
    }

    /// Subscribe to broadcasts. Returns a receiver for new messages.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.tx.subscribe()
    }
}

/// Handle a single WebSocket connection.
/// Sends a KeepAlive ping, then loops reading protobuf messages
/// and relaying broadcasts.
pub async fn handle_ws(
    socket: WebSocket,
    registry: ClientRegistry,
    asset_manager: AssetManager,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = registry.subscribe();

    // Send welcome ping
    let ping = Envelope {
        payload: Some(Payload::Ping(KeepAlive {
            timestamp: chrono::Utc::now().timestamp(),
        })),
    };
    let ping_data = ping.encode_to_vec();
    if let Err(e) = sender.send(Message::Binary(ping_data.into())).await {
        error!("Failed to send welcome ping: {}", e);
        return;
    }
    info!("WebSocket client connected, sent welcome ping");

    // Spawn broadcast relay task
    let relay = tokio::spawn(async move {
        while let Ok(data) = broadcast_rx.recv().await {
            if sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop: process incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                match Envelope::decode(&data[..]) {
                    Ok(envelope) => {
                        handle_message(&envelope, &asset_manager).await;
                    }
                    Err(e) => {
                        warn!("Failed to decode protobuf: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket client sent close");
                break;
            }
            Ok(_) => {} // Ignore text, ping, pong
            Err(e) => {
                debug!("WebSocket read error: {}", e);
                break;
            }
        }
    }

    relay.abort();
    info!("WebSocket client disconnected");
}

async fn handle_message(
    envelope: &Envelope,
    _asset_manager: &AssetManager,
) {
    match &envelope.payload {
        Some(Payload::ZellijAction(action)) => {
            handle_zellij_action(action).await;
        }
        Some(other) => {
            debug!("Received message: {:?}", std::mem::discriminant(other));
        }
        None => {}
    }
}

async fn handle_zellij_action(action: &zelland::ZellijAction) {
    info!("Executing Zellij action: {} on session: {}", action.action, action.session_name);
    
    let mut cmd = std::process::Command::new("zellij");
    cmd.arg("-s").arg(&action.session_name)
       .arg("action");
    
    for part in action.action.split_whitespace() {
        cmd.arg(part);
    }

    match cmd.status() {
        Ok(status) if status.success() => {
            info!("Zellij action executed successfully");
        }
        Ok(status) => {
            error!("Zellij action failed with status: {}", status);
        }
        Err(e) => {
            error!("Failed to execute Zellij command: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::zelland::{
        self, ClientStatus, OpenViewRequest,
    };

    #[test]
    fn test_broadcast_no_subscribers() {
        // Should not panic when there are no subscribers
        let registry = ClientRegistry::new();
        let envelope = Envelope {
            payload: Some(Payload::Ping(KeepAlive { timestamp: 42 })),
        };
        registry.broadcast(&envelope);
    }

    #[test]
    fn test_protobuf_roundtrip_keepalive() {
        let envelope = Envelope {
            payload: Some(Payload::Ping(KeepAlive { timestamp: 12345 })),
        };
        let data = envelope.encode_to_vec();
        let decoded = Envelope::decode(&data[..]).unwrap();
        match decoded.payload {
            Some(Payload::Ping(ka)) => assert_eq!(ka.timestamp, 12345),
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_protobuf_roundtrip_open_view() {
        let envelope = Envelope {
            payload: Some(Payload::OpenView(OpenViewRequest {
                asset_id: "abc123".into(),
                url: "/assets/abc123".into(),
                file_type: zelland::open_view_request::FileType::Markdown as i32,
                title: "README.md".into(),
            })),
        };
        let data = envelope.encode_to_vec();
        let decoded = Envelope::decode(&data[..]).unwrap();
        match decoded.payload {
            Some(Payload::OpenView(req)) => {
                assert_eq!(req.asset_id, "abc123");
                assert_eq!(req.file_type, zelland::open_view_request::FileType::Markdown as i32);
            }
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_protobuf_roundtrip_client_status() {
        let envelope = Envelope {
            payload: Some(Payload::Status(ClientStatus {
                state: zelland::client_status::ViewState::Viewer as i32,
                active_asset_id: "xyz".into(),
            })),
        };
        let data = envelope.encode_to_vec();
        let decoded = Envelope::decode(&data[..]).unwrap();
        match decoded.payload {
            Some(Payload::Status(s)) => {
                assert_eq!(s.state, zelland::client_status::ViewState::Viewer as i32);
                assert_eq!(s.active_asset_id, "xyz");
            }
            _ => panic!("Wrong payload type"),
        }
    }

    #[tokio::test]
    async fn test_broadcast_received_by_subscriber() {
        let registry = ClientRegistry::new();
        let mut rx = registry.subscribe();

        let envelope = Envelope {
            payload: Some(Payload::Ping(KeepAlive { timestamp: 77 })),
        };
        registry.broadcast(&envelope);

        let data = rx.recv().await.unwrap();
        let decoded = Envelope::decode(&data[..]).unwrap();
        match decoded.payload {
            Some(Payload::Ping(ka)) => assert_eq!(ka.timestamp, 77),
            _ => panic!("Wrong payload type"),
        }
    }
}
