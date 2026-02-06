pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/zelland.rs"));
}

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use crate::daemon::proto::{Envelope, envelope::Payload};
use prost::Message as _;
use tauri_plugin_notification::NotificationExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub name: Option<String>,
    pub host: String,
    pub session_name: String,
    pub root_path: String,
}

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
                        // Handle payloads without moving the envelope
                        match &envelope.payload {
                            Some(Payload::Notification(notif)) => {
                                let _ = app_handle.notification()
                                    .builder()
                                    .title(notif.title.clone())
                                    .body(notif.body.clone())
                                    .show();
                            }
                            _ => {
                                // Forward other events to frontend
                                let _ = app_handle.emit("daemon-event", envelope);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

#[tauri::command]
pub async fn daemon_get_projects(url: String) -> Result<Vec<Project>, String> {
    let client = reqwest::Client::new();
    let res = client.get(format!("{}/api/v1/projects", url))
        .send().await
        .map_err(|e| e.to_string())?;
    
    let projects = res.json::<Vec<Project>>().await
        .map_err(|e| e.to_string())?;
    
    Ok(projects)
}

#[tauri::command]
pub async fn daemon_activate_project(url: String, project_id: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client.post(format!("{}/api/v1/projects/activate", url))
        .json(&serde_json::json!({ "project_id": project_id }))
        .send().await
        .map_err(|e| e.to_string())?;
    
    if !res.status().is_success() {
        return Err(format!("Failed to activate project: {}", res.status()));
    }
    
    Ok(())
}

#[tauri::command]
pub async fn daemon_read_file(url: String, path: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let res = client.get(format!("{}/api/v1/fs/read", url))
        .query(&[("path", path)])
        .send().await
        .map_err(|e| e.to_string())?;
    
    let content = res.text().await
        .map_err(|e| e.to_string())?;
    
    Ok(content)
}