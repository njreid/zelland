use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use loro::{LoroDoc, LoroList, LoroMap, LoroValue, ToJson, ExportMode};
use tracing::{error, info};
use crate::store::{
    parse_markdown_comments, reify_markdown_comments, loro_cache_path, Annotation,
};
use crate::handlers::annotations::Comment;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnnState {
    pub id: String,
    pub quote: String,
    pub thread: Vec<Comment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codeblock_handle: Option<String>,
}

pub struct LoroManager {
    docs: Arc<Mutex<HashMap<PathBuf, DocState>>>,
}

struct DocState {
    doc: LoroDoc,
    broadcast_tx: broadcast::Sender<String>,
}

/// Extract annotation state from a LoroDoc into a JSON string of Vec<AnnState>.
/// Reads `annotations` (threads), `quotes`, `code_ranges`, and `codeblock_handles` maps.
fn doc_to_json(doc: &LoroDoc) -> String {
    let annotations_map = doc.get_map("annotations");
    let quotes_map = doc.get_map("quotes");
    let code_ranges_map = doc.get_map("code_ranges");
    let codeblock_handles_map = doc.get_map("codeblock_handles");

    let anns_json = annotations_map.get_deep_value().to_json();
    let quotes_json = quotes_map.get_deep_value().to_json();
    let code_ranges_json = code_ranges_map.get_deep_value().to_json();
    let codeblock_handles_json = codeblock_handles_map.get_deep_value().to_json();

    let quotes_val: serde_json::Value = serde_json::from_str(&quotes_json).unwrap_or_default();
    let code_ranges_val: serde_json::Value =
        serde_json::from_str(&code_ranges_json).unwrap_or_default();
    let codeblock_handles_val: serde_json::Value =
        serde_json::from_str(&codeblock_handles_json).unwrap_or_default();

    let mut anns: Vec<AnnState> = Vec::new();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&anns_json) {
        if let Some(map) = val.as_object() {
            for (id, thread_val) in map {
                let quote = quotes_val.get(id)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let code_range = code_ranges_val.get(id)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let codeblock_handle = codeblock_handles_val.get(id)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut thread = Vec::new();
                if let Some(list) = thread_val.as_array() {
                    for c_val in list {
                        if let Some(c_obj) = c_val.as_object() {
                            thread.push(Comment {
                                author: c_obj.get("author")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                timestamp: c_obj.get("timestamp")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                body: c_obj.get("body")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            });
                        }
                    }
                }
                anns.push(AnnState { id: id.clone(), quote, thread, code_range, codeblock_handle });
            }
        }
    }

    anns.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&anns).unwrap_or_else(|_| "[]".to_string())
}

/// Load or create a DocState for the given path. Must be called with the mutex held.
fn load_or_create_doc(path: &Path) -> DocState {
    let doc = LoroDoc::new();

    let cache_path = loro_cache_path(path);
    if cache_path.exists() {
        if let Ok(bytes) = std::fs::read(&cache_path) {
            if let Err(e) = doc.import(&bytes) {
                error!("Failed to import loro cache for {:?}: {}", path, e);
            } else {
                info!("Imported loro cache for {:?}", path);
            }
        }
    } else {
        // Initial populate from Markdown
        match parse_markdown_comments(path) {
            Ok(anns) => {
                info!("Parsed {} annotations from disk for {:?}", anns.len(), path);
                populate_doc_from_annotations(&doc, &anns);
            }
            Err(e) => {
                error!("Failed to parse markdown comments for {:?}: {}", path, e);
            }
        }
    }

    let (broadcast_tx, _) = broadcast::channel(100);
    DocState { doc, broadcast_tx }
}

/// Populate a LoroDoc from a slice of Annotations. Used on initial load from disk.
fn populate_doc_from_annotations(doc: &LoroDoc, anns: &[Annotation]) {
    let map = doc.get_map("annotations");
    let quotes_map = doc.get_map("quotes");
    let code_ranges_map = doc.get_map("code_ranges");
    let codeblock_handles_map = doc.get_map("codeblock_handles");

    for ann in anns {
        // Build thread as LoroValue::List
        let mut thread_list = Vec::new();
        for comment in &ann.thread {
            let mut c_map = HashMap::new();
            c_map.insert("author".to_string(), LoroValue::from(comment.author.clone()));
            c_map.insert("timestamp".to_string(), LoroValue::from(comment.timestamp.clone()));
            c_map.insert("body".to_string(), LoroValue::from(comment.body.clone()));
            thread_list.push(LoroValue::from(c_map));
        }
        map.insert(&ann.id, LoroValue::from(thread_list)).unwrap();

        if !ann.quote.is_empty() {
            quotes_map.insert(&ann.id, ann.quote.clone()).unwrap();
        }
        if let Some(ref range) = ann.code_range {
            code_ranges_map.insert(&ann.id, range.clone()).unwrap();
        }
        if let Some(ref handle) = ann.codeblock_handle {
            codeblock_handles_map.insert(&ann.id, handle.clone()).unwrap();
        }
    }
}

impl LoroManager {
    pub fn new() -> Self {
        Self {
            docs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Subscribe to a document. Returns (initial_json, broadcast_rx).
    pub async fn subscribe(&self, path: &Path) -> (String, broadcast::Receiver<String>) {
        let mut docs = self.docs.lock().await;
        let state = docs.entry(path.to_path_buf()).or_insert_with(|| load_or_create_doc(path));
        let initial_json = doc_to_json(&state.doc);
        let rx = state.broadcast_tx.subscribe();
        (initial_json, rx)
    }

    /// Add a new annotation (first comment in a new thread).
    /// If `code_range` and `codeblock_handle` are provided, this is a code block annotation.
    pub async fn add_annotation(
        &self,
        path: &Path,
        ann_id: String,
        quote: String,
        author: String,
        body: String,
        code_range: Option<String>,
        codeblock_handle: Option<String>,
    ) {
        let (json, snapshot_bytes, anns) = {
            let mut docs = self.docs.lock().await;
            let state = docs.entry(path.to_path_buf()).or_insert_with(|| load_or_create_doc(path));

            let quotes_map = state.doc.get_map("quotes");
            quotes_map.insert(&ann_id, quote.clone()).unwrap();

            if let Some(ref range) = code_range {
                let code_ranges_map = state.doc.get_map("code_ranges");
                code_ranges_map.insert(&ann_id, range.clone()).unwrap();
            }
            if let Some(ref handle) = codeblock_handle {
                let codeblock_handles_map = state.doc.get_map("codeblock_handles");
                codeblock_handles_map.insert(&ann_id, handle.clone()).unwrap();
            }

            let map = state.doc.get_map("annotations");
            let list = map.insert_container(&ann_id, LoroList::new()).unwrap();
            // Insert container first (attached), then write fields into it.
            let c_map = list.insert_container(0, LoroMap::new()).unwrap();
            c_map.insert("author", author).unwrap();
            c_map.insert("timestamp", chrono::Utc::now().to_rfc3339()).unwrap();
            c_map.insert("body", body).unwrap();

            let json = doc_to_json(&state.doc);
            let snapshot_bytes = state.doc.export(ExportMode::Snapshot).unwrap_or_default();
            let anns = extract_anns(&state.doc);
            let _ = state.broadcast_tx.send(json.clone());
            (json, snapshot_bytes, anns)
        };

        self.persist_data(path, &snapshot_bytes, &anns);
        drop(json);
    }

    /// Append a comment to an existing annotation thread.
    pub async fn add_comment(&self, path: &Path, ann_id: &str, author: String, body: String) {
        let (snapshot_bytes, anns) = {
            let mut docs = self.docs.lock().await;
            let state = docs.entry(path.to_path_buf()).or_insert_with(|| load_or_create_doc(path));

            let map = state.doc.get_map("annotations");
            let list = match map.get(ann_id) {
                Some(loro::ValueOrContainer::Container(loro::Container::List(list))) => list,
                _ => map.insert_container(ann_id, LoroList::new()).unwrap(),
            };

            let pos = list.len();
            let c_map = list.insert_container(pos, LoroMap::new()).unwrap();
            c_map.insert("author", author).unwrap();
            c_map.insert("timestamp", chrono::Utc::now().to_rfc3339()).unwrap();
            c_map.insert("body", body).unwrap();

            let json = doc_to_json(&state.doc);
            let snapshot_bytes = state.doc.export(ExportMode::Snapshot).unwrap_or_default();
            let anns = extract_anns(&state.doc);
            let _ = state.broadcast_tx.send(json);
            (snapshot_bytes, anns)
        };

        self.persist_data(path, &snapshot_bytes, &anns);
    }

    /// Delete an annotation (remove its thread, quote, code range, and handle).
    pub async fn delete_annotation(&self, path: &Path, ann_id: &str) {
        let (snapshot_bytes, anns) = {
            let mut docs = self.docs.lock().await;
            let state = docs.entry(path.to_path_buf()).or_insert_with(|| load_or_create_doc(path));

            let map = state.doc.get_map("annotations");
            if let Err(e) = map.delete(ann_id) {
                error!("Failed to delete annotation {} for {:?}: {}", ann_id, path, e);
            }
            let _ = state.doc.get_map("quotes").delete(ann_id);
            let _ = state.doc.get_map("code_ranges").delete(ann_id);
            let _ = state.doc.get_map("codeblock_handles").delete(ann_id);

            let json = doc_to_json(&state.doc);
            let snapshot_bytes = state.doc.export(ExportMode::Snapshot).unwrap_or_default();
            let anns = extract_anns(&state.doc);
            let _ = state.broadcast_tx.send(json);
            (snapshot_bytes, anns)
        };

        self.persist_data(path, &snapshot_bytes, &anns);
    }

    /// Refresh Loro document state from the Markdown file on disk.
    /// Called when the file watcher detects an external modification.
    pub async fn reload_from_disk(&self, path: &Path) {
        let mut docs = self.docs.lock().await;
        if let Some(state) = docs.get_mut(path) {
            if let Ok(anns) = parse_markdown_comments(path) {
                let map = state.doc.get_map("annotations");
                let quotes_map = state.doc.get_map("quotes");
                let code_ranges_map = state.doc.get_map("code_ranges");
                let codeblock_handles_map = state.doc.get_map("codeblock_handles");

                for ann in anns.into_iter() {
                    // Skip if already tracked in Loro (daemon is authoritative for its entries).
                    // Code annotations use deterministic IDs, so this check works for them too.
                    if map.get(&ann.id).is_some() {
                        continue;
                    }

                    let mut thread_list = Vec::new();
                    for comment in &ann.thread {
                        let mut c_map = HashMap::new();
                        c_map.insert("author".to_string(), LoroValue::from(comment.author.clone()));
                        c_map.insert("timestamp".to_string(), LoroValue::from(comment.timestamp.clone()));
                        c_map.insert("body".to_string(), LoroValue::from(comment.body.clone()));
                        thread_list.push(LoroValue::from(c_map));
                    }
                    map.insert(&ann.id, LoroValue::from(thread_list)).unwrap();

                    if !ann.quote.is_empty() {
                        quotes_map.insert(&ann.id, ann.quote).unwrap();
                    }
                    if let Some(range) = ann.code_range {
                        code_ranges_map.insert(&ann.id, range).unwrap();
                    }
                    if let Some(handle) = ann.codeblock_handle {
                        codeblock_handles_map.insert(&ann.id, handle).unwrap();
                    }
                }

                let json = doc_to_json(&state.doc);
                let _ = state.broadcast_tx.send(json);

                let snapshot_bytes = state.doc.export(ExportMode::Snapshot).unwrap_or_default();
                let cache_path = loro_cache_path(path);
                let _ = std::fs::write(&cache_path, snapshot_bytes);
            }
        }
    }

    /// Persist snapshot bytes and reify to Markdown. Called after the mutex is released.
    fn persist_data(&self, path: &Path, snapshot_bytes: &[u8], anns: &[Annotation]) {
        let cache_path = loro_cache_path(path);
        if let Err(e) = std::fs::write(&cache_path, snapshot_bytes) {
            error!("Failed to save loro cache for {:?}: {}", path, e);
        }
        if let Err(e) = reify_markdown_comments(path, anns) {
            error!("Failed to reify markdown for {:?}: {}", path, e);
        }
    }
}

/// Extract the full annotation state from a doc as `Vec<Annotation>` for markdown reification.
fn extract_anns(doc: &LoroDoc) -> Vec<Annotation> {
    let annotations_map = doc.get_map("annotations");
    let quotes_map = doc.get_map("quotes");
    let code_ranges_map = doc.get_map("code_ranges");
    let codeblock_handles_map = doc.get_map("codeblock_handles");

    let anns_json = annotations_map.get_deep_value().to_json();
    let quotes_json = quotes_map.get_deep_value().to_json();
    let code_ranges_json = code_ranges_map.get_deep_value().to_json();
    let codeblock_handles_json = codeblock_handles_map.get_deep_value().to_json();

    let quotes_val: serde_json::Value = serde_json::from_str(&quotes_json).unwrap_or_default();
    let code_ranges_val: serde_json::Value =
        serde_json::from_str(&code_ranges_json).unwrap_or_default();
    let codeblock_handles_val: serde_json::Value =
        serde_json::from_str(&codeblock_handles_json).unwrap_or_default();

    let mut anns: Vec<Annotation> = Vec::new();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&anns_json) {
        if let Some(map) = val.as_object() {
            for (id, thread_val) in map {
                let quote = quotes_val.get(id)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let code_range = code_ranges_val.get(id)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let codeblock_handle = codeblock_handles_val.get(id)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut thread = Vec::new();
                if let Some(list) = thread_val.as_array() {
                    for c_val in list {
                        if let Some(c_obj) = c_val.as_object() {
                            thread.push(Comment {
                                author: c_obj.get("author")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                timestamp: c_obj.get("timestamp")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                                body: c_obj.get("body")
                                    .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            });
                        }
                    }
                }

                anns.push(Annotation {
                    id: id.clone(),
                    quote,
                    thread,
                    code_range,
                    codeblock_handle,
                });
            }
        }
    }

    anns
}

