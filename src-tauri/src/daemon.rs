pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/zelland.rs"));
}

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use tauri::{AppHandle, Emitter};
use crate::daemon::proto::{Envelope, envelope::Payload, ZellijAction};
use prost::Message as _;
use tauri_plugin_notification::NotificationExt;
use serde::{Deserialize, Serialize};
use log::{info, error, debug};
use tokio::sync::Mutex;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentSessionEntry {
    pub session_name: String,
    pub connected_at: String,
    pub readme_excerpt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub name: Option<String>,
    pub host: String,
    pub session_name: String,
    pub root_path: String,
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

pub struct DaemonManager {
    pub ws_write: Arc<Mutex<Option<WsSink>>>,
    pub http_client: reqwest::Client,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            ws_write: Arc::new(Mutex::new(None)),
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn connect(&self, url: String, app_handle: AppHandle) -> Result<(), String> {
        info!("Connecting to daemon at {}", url);
        let (ws_stream, _) = connect_async(url.clone()).await
            .map_err(|e| {
                error!("WebSocket connection failed to {}: {}", url, e);
                format!("WebSocket connection failed: {}", e)
            })?;

        let (write, mut read) = ws_stream.split();
        
        {
            let mut ws_write = self.ws_write.lock().await;
            *ws_write = Some(write);
        }

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
                                    info!("Received notification: {} (source={})", notif.title, notif.source);

                                    // OS notification — on Android the JS side calls
                                    // show_agent_notification (rich, with action button) when
                                    // the app is backgrounded, so we skip the basic one there.
                                    #[cfg(not(target_os = "android"))]
                                    let _ = app_handle.notification()
                                        .builder()
                                        .title(&notif.title)
                                        .body(if !notif.question.is_empty() { &notif.question } else { &notif.body })
                                        .show();

                                    // In-app event with full context
                                    #[derive(Serialize, Clone)]
                                    struct AgentNotifEvent {
                                        title: String,
                                        body: String,
                                        question: String,
                                        source: i32,
                                        session_name: Option<String>,
                                        tab_index: u32,
                                        pane_id: u32,
                                    }
                                    let ev = AgentNotifEvent {
                                        title: notif.title.clone(),
                                        body: notif.body.clone(),
                                        question: notif.question.clone(),
                                        source: notif.source,
                                        session_name: notif.target.as_ref().map(|t| t.session_name.clone()),
                                        tab_index: notif.target.as_ref().map(|t| t.tab_index).unwrap_or(0),
                                        pane_id: notif.target.as_ref().map(|t| t.pane_id).unwrap_or(0),
                                    };
                                    let _ = app_handle.emit("agent-notification", ev);
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

    pub async fn send_action(&self, action: String, session_name: String) -> Result<(), String> {
        let envelope = Envelope {
            payload: Some(Payload::ZellijAction(ZellijAction {
                action,
                session_name,
            })),
        };

        let buf = envelope.encode_to_vec();

        let mut ws_write = self.ws_write.lock().await;
        if let Some(write) = ws_write.as_mut() {
            write.send(Message::Binary(buf.into())).await
                .map_err(|e| format!("Failed to send message: {}", e))?;
            Ok(())
        } else {
            Err("Daemon not connected".to_string())
        }
    }
}

#[tauri::command]
pub async fn daemon_get_projects(state: tauri::State<'_, DaemonManager>, url: String) -> Result<Vec<Project>, String> {
    debug!("Fetching projects from daemon: {}", url);
    let res = state.http_client.get(format!("{}/api/v1/projects", url))
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
pub async fn daemon_activate_project(state: tauri::State<'_, DaemonManager>, url: String, project_id: String) -> Result<(), String> {
    info!("Activating project {} via daemon at {}", project_id, url);
    let res = state.http_client.post(format!("{}/api/v1/projects/activate", url))
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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct ProjectFiles {
    pub root_md: Vec<String>,
    pub docs_md: Vec<String>,
}

#[tauri::command]
pub async fn daemon_list_project_files(state: tauri::State<'_, DaemonManager>, url: String, project_id: String) -> Result<ProjectFiles, String> {
    debug!("Listing files for project {} from daemon: {}", project_id, url);
    let res = state.http_client
        .get(format!("{}/api/v1/projects/{}/files", url, project_id))
        .send().await
        .map_err(|e| { error!("Failed to list project files: {}", e); e.to_string() })?;
    res.json::<ProjectFiles>().await
        .map_err(|e| { error!("Failed to parse project files: {}", e); e.to_string() })
}

#[tauri::command]
pub async fn daemon_read_file(state: tauri::State<'_, DaemonManager>, url: String, path: String) -> Result<String, String> {
    debug!("Reading file via daemon: {} from {}", path, url);
    let res = state.http_client.get(format!("{}/api/v1/fs/read", url))
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

#[tauri::command]
pub async fn daemon_get_recent_sessions(
    state: tauri::State<'_, DaemonManager>,
    url: String,
) -> Result<Vec<RecentSessionEntry>, String> {
    let res = state.http_client
        .get(format!("{}/api/v1/sessions/recent", url))
        .send().await
        .map_err(|e| e.to_string())?;
    res.json::<Vec<RecentSessionEntry>>().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn daemon_record_session(
    state: tauri::State<'_, DaemonManager>,
    url: String,
    session_name: String,
) -> Result<(), String> {
    let res = state.http_client
        .post(format!("{}/api/v1/sessions/recent", url))
        .json(&serde_json::json!({ "session_name": session_name }))
        .send().await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Failed: {}", res.status()));
    }
    Ok(())
}

#[tauri::command]
pub async fn daemon_annotate_file(
    state: tauri::State<'_, DaemonManager>,
    url: String,
    path: String,
    ann_id: String,
    quote: String,
    author: String,
    body: String,
    code_range: Option<String>,
    codeblock_handle: Option<String>,
    code_text_prefix: Option<String>,
) -> Result<(), String> {
    debug!("Annotating file via daemon: {} for ann {}", path, ann_id);
    let res = state.http_client.post(format!("{}/api/v1/fs/annotate", url))
        .json(&serde_json::json!({
            "path": path,
            "ann_id": ann_id,
            "quote": quote,
            "author": author,
            "body": body,
            "code_range": code_range,
            "codeblock_handle": codeblock_handle,
            "code_text_prefix": code_text_prefix,
        }))
        .send().await
        .map_err(|e| {
            error!("Failed to send annotate request for {}: {}", path, e);
            e.to_string()
        })?;

    if !res.status().is_success() {
        let err = format!("Failed to annotate file: status {}", res.status());
        error!("{}", err);
        return Err(err);
    }

    Ok(())
}

#[tauri::command]
pub async fn daemon_mutate_file(
    state: tauri::State<'_, DaemonManager>,
    url: String,
    path: String,
    ann_id: String,
    quote: String,
    prefix: String,
    suffix: String,
) -> Result<(), String> {
    debug!("Mutating file via daemon: {} for ann {}", path, ann_id);
    let res = state.http_client.post(format!("{}/api/v1/fs/mutate", url))
        .json(&serde_json::json!({
            "path": path,
            "ann_id": ann_id,
            "quote": quote,
            "prefix": prefix,
            "suffix": suffix,
        }))
        .send().await
        .map_err(|e| {
            error!("Failed to send mutate request for {}: {}", path, e);
            e.to_string()
        })?;
    
    if !res.status().is_success() {
        let err = format!("Failed to mutate file: status {}", res.status());
        error!("{}", err);
        return Err(err);
    }
    
    Ok(())
}
