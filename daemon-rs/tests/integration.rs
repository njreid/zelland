use prost::Message;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use zelland_daemon::assets::AssetManager;
use zelland_daemon::config::Config;
use zelland_daemon::proto::zelland::{envelope::Payload, Envelope};
use zelland_daemon::server::{build_router, AppState};
use zelland_daemon::watcher::WatchCommand;
use zelland_daemon::ws::ClientRegistry;

/// Start the server on a random port and return the address.
async fn start_test_server() -> (SocketAddr, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    // Create project dirs
    std::fs::create_dir(dir.path().join("project-a")).unwrap();
    std::fs::create_dir(dir.path().join("project-b")).unwrap();
    // Create a test file
    std::fs::write(dir.path().join("project-a").join("README.md"), "# Hello\n").unwrap();

    let config = Config {
        port: 0,
        cert_file: None,
        key_file: None,
        projects_path: dir.path().to_path_buf(),
    };

    let (watcher_tx, _rx) = mpsc::channel::<WatchCommand>(16);

    let state = AppState {
        config: Arc::new(config),
        asset_manager: AssetManager::new(),
        registry: ClientRegistry::new(),
        asset_paths: Arc::new(RwLock::new(HashMap::new())),
        watcher_tx,
    };

    let app = build_router(state).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, dir)
}

#[tokio::test]
async fn test_full_api_flow() {
    let (addr, _dir) = start_test_server().await;
    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    // 1. List projects
    let resp = client.get(format!("{}/api/v1/projects", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let projects: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(projects.len(), 2);

    // 2. Activate project
    let resp = client
        .post(format!("{}/api/v1/projects/activate", base))
        .json(&serde_json::json!({"project_id": "project-a"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 3. Read file
    let readme_path = _dir.path().join("project-a").join("README.md");
    let resp = client
        .get(format!(
            "{}/api/v1/fs/read?path={}",
            base,
            readme_path.to_string_lossy()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "# Hello\n");

    // 4. Asset 404
    let resp = client
        .get(format!("{}/assets/nonexistent", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_websocket_ping() {
    let (addr, _dir) = start_test_server().await;
    let ws_url = format!("ws://{}/ws", addr);

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    use futures_util::StreamExt;
    let (_, mut read) = ws_stream.split();

    // Should receive a welcome ping
    if let Some(Ok(msg)) = read.next().await {
        let data = msg.into_data();
        let envelope = Envelope::decode(&data[..]).unwrap();
        match envelope.payload {
            Some(Payload::Ping(ka)) => {
                assert!(ka.timestamp > 0);
            }
            _ => panic!("Expected KeepAlive ping, got {:?}", envelope.payload),
        }
    } else {
        panic!("No message received");
    }
}

#[tokio::test]
async fn test_trigger_from_loopback() {
    let (addr, dir) = start_test_server().await;
    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    // Create a file to trigger
    let file = dir.path().join("test.md");
    std::fs::write(&file, "# Test").unwrap();

    let resp = client
        .post(format!("{}/api/v1/trigger/md", base))
        .json(&serde_json::json!({
            "path": file.to_string_lossy(),
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(text.contains("Showing"));
}
