use futures_util::{SinkExt, StreamExt};
use prost::Message;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Map, ReadTxn, Transact};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use zelland_daemon::assets::AssetManager;
use zelland_daemon::config::Config;
use zelland_daemon::proto::zelland::{envelope::Payload, Envelope};
use zelland_daemon::server::{build_router, AppState};
use zelland_daemon::store::{Ann, Comment, Selector};
use zelland_daemon::watcher::WatchCommand;
use zelland_daemon::ws::ClientRegistry;
use zelland_daemon::yjs::DocManager;

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
        watcher_tx,
        doc_manager: DocManager::new(),
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
    let resp = client
        .get(format!(
            "{}/api/v1/fs/read?path=project-a/README.md",
            base
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
            "path": "test.md",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(text.contains("Showing"));
}

#[tokio::test]
async fn test_annotation_rest_roundtrip() {
    let (addr, dir) = start_test_server().await;
    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    let anns = vec![Ann {
        id: "test1".into(),
        selector: Selector {
            quote: "hello world".into(),
            prefix: "say ".into(),
            suffix: " now".into(),
        },
        thread: vec![Comment {
            id: "c1".into(),
            author: "alice".into(),
            created: "2026-02-11T10:00:00Z".into(),
            body: "A comment".into(),
        }],
    }];

    // PUT annotations
    let resp = client
        .put(format!("{}/annotations/project-a/README.md", base))
        .json(&anns)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify .ann.kdl file was created on disk
    let ann_file = dir.path().join("project-a").join("README.ann.kdl");
    assert!(ann_file.exists());

    // GET annotations
    let resp = client
        .get(format!("{}/annotations/project-a/README.md", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let result: Vec<Ann> = resp.json().await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "test1");
    assert_eq!(result[0].selector.quote, "hello world");
    assert_eq!(result[0].thread[0].body, "A comment");
}

#[tokio::test]
async fn test_annotation_ws_sync() {
    let (addr, dir) = start_test_server().await;

    // Write an .ann.kdl file on disk first
    let ann_file = dir.path().join("project-a").join("README.ann.kdl");
    let anns = vec![Ann {
        id: "ws-test".into(),
        selector: Selector {
            quote: "test text".into(),
            prefix: "".into(),
            suffix: "".into(),
        },
        thread: vec![],
    }];
    zelland_daemon::store::save_anns(&ann_file, &anns).unwrap();

    // Connect to sync endpoint
    let ws_url = format!("ws://{}/annotations/sync/project-a/README.md", addr);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Should receive SyncStep1 from server (its state vector)
    let msg = read.next().await.unwrap().unwrap();
    let data = msg.into_data();
    assert!(!data.is_empty());
    // First byte should be 0 (MSG_SYNC), second byte should be 0 (SYNC_STEP1)
    assert_eq!(data[0], 0); // MSG_SYNC
    assert_eq!(data[1], 0); // SYNC_STEP1

    // Send our SyncStep1 (empty state vector → request full state)
    let empty_sv = yrs::StateVector::default().encode_v1();
    let step1 = zelland_daemon::sync::encode_sync_step1(&empty_sv);
    write
        .send(tokio_tungstenite::tungstenite::Message::Binary(step1.into()))
        .await
        .unwrap();

    // Should receive SyncStep2 from server (full document update)
    let msg = read.next().await.unwrap().unwrap();
    let data = msg.into_data();
    assert!(!data.is_empty());
    assert_eq!(data[0], 0); // MSG_SYNC
    assert_eq!(data[1], 1); // SYNC_STEP2

    // Decode the update and apply to a local doc
    let decoded = zelland_daemon::sync::decode_message(&data).unwrap();
    match decoded {
        zelland_daemon::sync::SyncMessage::SyncStep2(update_bytes) => {
            let doc = yrs::Doc::new();
            {
                let mut txn = doc.transact_mut();
                let update = yrs::Update::decode_v1(&update_bytes).unwrap();
                txn.apply_update(update).unwrap();
            }

            // Verify the doc has our annotation
            let txn = doc.transact();
            let map = txn.get_map("annotations").unwrap();
            // The map should have our "ws-test" annotation
            assert!(map.get(&txn, "ws-test").is_some());
        }
        other => panic!("Expected SyncStep2, got {:?}", other),
    }

    // Close
    write
        .send(tokio_tungstenite::tungstenite::Message::Close(None))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mutate_source_file() {
    let (addr, dir) = start_test_server().await;
    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    let md_path = dir.path().join("project-a").join("README.md");
    // start_test_server writes "# Hello\n" to this file
    
    let resp = client
        .post(format!("{}/api/v1/fs/mutate", base))
        .json(&serde_json::json!({
            "path": "project-a/README.md",
            "ann_id": "test-123",
            "quote": "Hello",
            "prefix": "# ",
            "suffix": "\n"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify content
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert_eq!(content, "# [|test-123|]Hello\n");

    // Test idempotency
    let resp = client
        .post(format!("{}/api/v1/fs/mutate", base))
        .json(&serde_json::json!({
            "path": "project-a/README.md",
            "ann_id": "test-123",
            "quote": "Hello",
            "prefix": "# ",
            "suffix": "\n"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert_eq!(content, "# [|test-123|]Hello\n"); // No duplicate marker
}
