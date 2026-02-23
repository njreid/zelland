use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use loro::{LoroDoc, LoroList, LoroMap, ToJson, ExportMode};
use tracing::{error};
use crate::store::{parse_markdown_comments, reify_markdown_comments, loro_cache_path};
use crate::handlers::annotations::Comment;

pub struct LoroManager {
    docs: Arc<Mutex<HashMap<PathBuf, DocState>>>,
}

struct DocState {
    doc: LoroDoc,
    broadcast_tx: broadcast::Sender<Vec<u8>>,
}

impl LoroManager {
    pub fn new() -> Self {
        Self {
            docs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_or_create(&self, path: &Path) -> (LoroDoc, broadcast::Receiver<Vec<u8>>) {
        let mut docs = self.docs.lock().await;
        if let Some(state) = docs.get(path) {
            return (state.doc.fork(), state.broadcast_tx.subscribe());
        }

        let doc = LoroDoc::new();
        
        // Try to load from binary cache first
        let cache_path = loro_cache_path(path);
        if cache_path.exists() {
            if let Ok(bytes) = std::fs::read(&cache_path) {
                if let Err(e) = doc.import(&bytes) {
                    error!("Failed to import loro cache for {:?}: {}", path, e);
                }
            }
        } else {
            // Initial populate from Markdown
            if let Ok(anns) = parse_markdown_comments(path) {
                let map = doc.get_map("annotations");
                for (id, thread) in anns.into_iter() {
                    let list = map.insert_container(&id, LoroList::new()).unwrap();
                    for comment in thread {
                        let c_map = LoroMap::new();
                        c_map.insert("author", comment.author).unwrap();
                        c_map.insert("timestamp", comment.timestamp).unwrap();
                        c_map.insert("body", comment.body).unwrap();
                        list.insert_container(list.len(), c_map).unwrap();
                    }
                }
            }
        }

        let (broadcast_tx, _) = broadcast::channel(100);
        let state = DocState {
            doc: doc.clone(),
            broadcast_tx: broadcast_tx.clone(),
        };
        
        docs.insert(path.to_path_buf(), state);
        (doc, broadcast_tx.subscribe())
    }

    pub async fn add_annotation(&self, path: &Path, ann_id: String, author: String, body: String) {
        let (doc, _) = self.get_or_create(path).await;
        let map = doc.get_map("annotations");
        let list = map.insert_container(&ann_id, LoroList::new()).unwrap();
        
        let c_map = LoroMap::new();
        c_map.insert("author", author).unwrap();
        c_map.insert("timestamp", chrono::Utc::now().to_rfc3339()).unwrap();
        c_map.insert("body", body).unwrap();
        list.insert_container(0, c_map).unwrap();

        // Export update to notify others
        let update = doc.export(ExportMode::Updates { from: Default::default() }).unwrap();
        let docs = self.docs.lock().await;
        if let Some(state) = docs.get(path) {
            let _ = state.broadcast_tx.send(update);
        }

        self.persist(path, &doc).await;
    }

    pub async fn apply_update(&self, path: &Path, update: &[u8]) {
        let docs = self.docs.lock().await;
        if let Some(state) = docs.get(path) {
            if let Err(e) = state.doc.import(update) {
                error!("Failed to apply loro update for {:?}: {}", path, e);
                return;
            }
            
            // Broadcast to other clients
            let _ = state.broadcast_tx.send(update.to_vec());
            
            // Reify to disk
            self.persist(path, &state.doc).await;
        }
    }

    async fn persist(&self, path: &Path, doc: &LoroDoc) {
        // Save binary cache
        let cache_path = loro_cache_path(path);
        let bytes = doc.export(ExportMode::Snapshot).unwrap();
        if let Err(e) = std::fs::write(&cache_path, bytes) {
            error!("Failed to save loro cache for {:?}: {}", path, e);
        }

        // Reify to Markdown
        let annotations_map = doc.get_map("annotations");
        let mut anns: Vec<(String, Vec<Comment>)> = Vec::new();
        
        let json = annotations_map.get_value().to_json();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(map) = val.as_object() {
                for (id, thread_val) in map {
                    let mut thread = Vec::new();
                    if let Some(list) = thread_val.as_array() {
                        for c_val in list {
                            if let Some(c_obj) = c_val.as_object() {
                                thread.push(Comment {
                                    author: c_obj.get("author").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                    timestamp: c_obj.get("timestamp").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                    body: c_obj.get("body").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                });
                            }
                        }
                    }
                    anns.push((id.clone(), thread));
                }
            }
        }

        if let Err(e) = reify_markdown_comments(path, &anns) {
            error!("Failed to reify markdown for {:?}: {}", path, e);
        }
    }
}
