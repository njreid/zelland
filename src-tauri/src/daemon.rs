pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/zelland.rs"));
}

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use crate::daemon::proto::Envelope;
use prost::Message as _;

pub struct DaemonManager {
    app_handle: AppHandle,
}

impl DaemonManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub async fn connect(&self, url: String) -> Result<(), String> {
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| format!("WebSocket connection failed: {}", e))?;

        let (_, mut read) = ws_stream.split();
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                if let Ok(Message::Binary(data)) = msg {
                    if let Ok(envelope) = Envelope::decode(&data[..]) {
                        // Forward to frontend
                        let _ = app_handle.emit("daemon-event", envelope);
                    }
                }
            }
        });

        Ok(())
    }
}