use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::proto::zelland::{self, envelope::Payload, Envelope, KeepAlive};
use crate::store;

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
    asset_paths: Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
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
                        handle_message(&envelope, &asset_paths).await;
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
    asset_paths: &Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
) {
    match &envelope.payload {
        Some(Payload::Annotation(action)) => {
            handle_annotation(action, asset_paths).await;
        }
        Some(other) => {
            debug!("Received message: {:?}", std::mem::discriminant(other));
        }
        None => {}
    }
}

async fn handle_annotation(
    action: &zelland::AnnotationAction,
    asset_paths: &Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
) {
    let paths = asset_paths.read().await;
    let file_path = paths
        .get(&action.file_path)
        .cloned()
        .unwrap_or_else(|| action.file_path.clone());
    drop(paths);

    let data = match &action.data {
        Some(d) => d,
        None => {
            warn!("Annotation action missing data");
            return;
        }
    };

    // Derive KDL path from file path
    let kdl_path = if let Some(pos) = file_path.rfind('.') {
        format!("{}.kdl", &file_path[..pos])
    } else {
        format!("{}.kdl", file_path)
    };

    let ann = store::Annotation {
        id: data.id.clone(),
        user: String::new(),
        timestamp: data.timestamp,
        context_hash: data.context_hash.clone(),
        target_text: data.target_text.clone(),
        body: data.body.clone(),
    };

    if let Err(e) = store::append_annotation(std::path::Path::new(&kdl_path), ann) {
        error!("Failed to save annotation to {}: {}", kdl_path, e);
    } else {
        info!("Saved annotation to {}", kdl_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::zelland::{
        self, AnnotationAction, AnnotationData, ClientStatus, OpenViewRequest,
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
    fn test_protobuf_roundtrip_annotation() {
        let envelope = Envelope {
            payload: Some(Payload::Annotation(AnnotationAction {
                r#type: zelland::annotation_action::ActionType::Create as i32,
                file_path: "test.md".into(),
                data: Some(AnnotationData {
                    id: "ann-1".into(),
                    target_text: "hello".into(),
                    context_hash: "sha256:abc".into(),
                    body: "note".into(),
                    timestamp: 999,
                }),
            })),
        };
        let data = envelope.encode_to_vec();
        let decoded = Envelope::decode(&data[..]).unwrap();
        match decoded.payload {
            Some(Payload::Annotation(action)) => {
                assert_eq!(action.file_path, "test.md");
                assert_eq!(action.data.unwrap().body, "note");
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
