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
use log::{info, error, debug};

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
        info!("Connecting to daemon at {}", url);
        let (ws_stream, _) = connect_async(url.clone()).await
            .map_err(|e| {
                error!("WebSocket connection failed to {}: {}", url, e);
                format!("WebSocket connection failed: {}", e)
            })?;

        let (_, mut read) = ws_stream.split();
        let app_handle = self.app_handle.clone();
        info!("Daemon connection established: {}", url);

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        debug!("Received binary message from daemon ({} bytes)", data.len());
                        if let Ok(envelope) = Envelope::decode(&data[..]) {
                            // Handle payloads without moving the envelope
                            match &envelope.payload {
                                Some(Payload::Notification(notif)) => {
                                    info!("Received notification from daemon: {}", notif.title);
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
                        } else {
                            error!("Failed to decode protobuf envelope from daemon");
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("Daemon WebSocket error: {}", e);
                        break;
                    }
                }
            }
            info!("Daemon connection closed for {}", url);
        });

        Ok(())
    }
}

#[tauri::command]
pub async fn daemon_get_projects(url: String) -> Result<Vec<Project>, String> {
    debug!("Fetching projects from daemon: {}", url);
    let client = reqwest::Client::new();
    let res = client.get(format!("{}/api/v1/projects", url))
        .send().await
        .map_err(|e| {
            error!("Failed to fetch projects from {}: {}", url, e);
            e.to_string()
        })?;
    
    let projects = res.json::<Vec<Project>>().await
        .map_err(|e| {
            error!("Failed to parse projects JSON from {}: {}", url, e);
            e.to_string()
        })?;
    
    debug!("Successfully fetched {} projects from {}", projects.len(), url);
    Ok(projects)
}

#[tauri::command]
pub async fn daemon_activate_project(url: String, project_id: String) -> Result<(), String> {
    info!("Activating project {} via daemon at {}", project_id, url);
    let client = reqwest::Client::new();
    let res = client.post(format!("{}/api/v1/projects/activate", url))
        .json(&serde_json::json!({ "project_id": project_id }))
        .send().await
        .map_err(|e| {
            error!("Failed to send activate project request for {}: {}", project_id, e);
            e.to_string()
        })?;
    
    if !res.status().is_success() {
        let err = format!("Failed to activate project: status {}", res.status());
        error!("{}", err);
        return Err(err);
    }
    
    Ok(())
}

#[tauri::command]
pub async fn daemon_read_file(url: String, path: String) -> Result<String, String> {
    debug!("Reading file via daemon: {} from {}", path, url);
    let client = reqwest::Client::new();
    let res = client.get(format!("{}/api/v1/fs/read", url))
        .query(&[("path", path.clone())])
        .send().await
        .map_err(|e| {
            error!("Failed to read file {} via daemon {}: {}", path, url, e);
            e.to_string()
        })?;
    
    if !res.status().is_success() {
        let err = format!("Failed to read file: status {}", res.status());
        error!("{}", err);
        return Err(err);
    }
    
    let content = res.text().await
        .map_err(|e| {
            error!("Failed to get text content for {}: {}", path, e);
            e.to_string()
        })?;
    
    Ok(content)
}
