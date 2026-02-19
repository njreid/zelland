/// YJS document manager — manages one `yrs::Doc` per annotation file.
///
/// Handles:
/// - Loading KDL from disk into a fresh YJS Doc (cold start)
/// - Per-document broadcast channel for connected peers
/// - Debounced persistence (5s after last change, on disconnect, on shutdown)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Array, Doc, Map, MapPrelim, Out, ReadTxn, Transact, WriteTxn};

use crate::store::{self, Ann, Comment, Selector};

const BROADCAST_CAPACITY: usize = 256;
const FLUSH_DEBOUNCE: Duration = Duration::from_secs(5);

/// Per-document state.
pub(crate) struct DocState {
    doc: Doc,
    update_tx: broadcast::Sender<Vec<u8>>,
    file_path: PathBuf,
    flush_handle: Option<JoinHandle<()>>,
    dirty: bool,
}

#[derive(Debug)]
pub enum DocError {
    Store(store::StoreError),
    Yrs(String),
}

impl std::fmt::Display for DocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocError::Store(e) => write!(f, "store error: {}", e),
            DocError::Yrs(e) => write!(f, "yrs error: {}", e),
        }
    }
}

impl std::error::Error for DocError {}

impl From<store::StoreError> for DocError {
    fn from(e: store::StoreError) -> Self {
        DocError::Store(e)
    }
}

/// Manages YJS documents keyed by file path.
#[derive(Clone)]
pub struct DocManager {
    docs: Arc<RwLock<HashMap<String, Arc<Mutex<DocState>>>>>,
}

impl DocManager {
    pub fn new() -> Self {
        Self {
            docs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a DocState for the given file path.
    /// On first access, loads the `.ann.kdl` file from disk and populates the Doc.
    pub(crate) async fn get_or_create(&self, file_path: &str) -> Arc<Mutex<DocState>> {
        // Fast path: check if already exists
        {
            let docs = self.docs.read().await;
            if let Some(state) = docs.get(file_path) {
                return state.clone();
            }
        }

        // Slow path: create new
        let mut docs = self.docs.write().await;
        // Double-check after acquiring write lock
        if let Some(state) = docs.get(file_path) {
            return state.clone();
        }

        let ann_path = store::ann_kdl_path(Path::new(file_path));
        let anns = store::load_anns(&ann_path).unwrap_or_default();

        let doc = Doc::new();
        populate_doc(&doc, &anns);

        let (update_tx, _) = broadcast::channel(BROADCAST_CAPACITY);

        let state = Arc::new(Mutex::new(DocState {
            doc,
            update_tx,
            file_path: ann_path,
            flush_handle: None,
            dirty: false,
        }));

        docs.insert(file_path.to_string(), state.clone());
        state
    }

    /// Subscribe to updates for a specific document.
    pub async fn subscribe(&self, file_path: &str) -> broadcast::Receiver<Vec<u8>> {
        let state = self.get_or_create(file_path).await;
        let locked = state.lock().await;
        locked.update_tx.subscribe()
    }

    /// Get the state vector for a document (for SyncStep1).
    pub async fn state_vector(&self, file_path: &str) -> Result<Vec<u8>, DocError> {
        let state = self.get_or_create(file_path).await;
        let locked = state.lock().await;
        let txn = locked.doc.transact();
        Ok(txn.state_vector().encode_v1())
    }

    /// Compute the update diff for a client's state vector (for SyncStep2).
    pub async fn encode_diff(
        &self,
        file_path: &str,
        state_vector: &[u8],
    ) -> Result<Vec<u8>, DocError> {
        let state = self.get_or_create(file_path).await;
        let locked = state.lock().await;
        let sv = yrs::StateVector::decode_v1(state_vector)
            .map_err(|e| DocError::Yrs(format!("invalid state vector: {}", e)))?;
        let txn = locked.doc.transact();
        Ok(txn.encode_state_as_update_v1(&sv))
    }

    /// Apply an incoming update from a client to the document.
    /// Broadcasts the update to all other peers and schedules a debounced flush.
    pub async fn apply_update(&self, file_path: &str, update: &[u8]) -> Result<(), DocError> {
        let state = self.get_or_create(file_path).await;
        let mut locked = state.lock().await;

        {
            let mut txn = locked.doc.transact_mut();
            let decoded = yrs::Update::decode_v1(update)
                .map_err(|e| DocError::Yrs(format!("invalid update: {}", e)))?;
            txn.apply_update(decoded)
                .map_err(|e| DocError::Yrs(format!("failed to apply update: {}", e)))?;
        }

        // Broadcast to all peers
        let _ = locked.update_tx.send(update.to_vec());

        // Schedule debounced flush
        self.schedule_flush(&mut locked, file_path.to_string());

        Ok(())
    }

    /// Read annotations from the YJS doc as Vec<Ann> (for REST GET endpoint).
    pub async fn read_annotations(&self, file_path: &str) -> Result<Vec<Ann>, DocError> {
        let state = self.get_or_create(file_path).await;
        let locked = state.lock().await;
        Ok(read_doc(&locked.doc))
    }

    /// Flush a specific document to disk immediately.
    pub async fn flush(&self, file_path: &str) -> Result<(), DocError> {
        let docs = self.docs.read().await;
        if let Some(state) = docs.get(file_path) {
            let mut locked = state.lock().await;
            if locked.dirty {
                let anns = read_doc(&locked.doc);
                store::save_anns(&locked.file_path, &anns)?;
                locked.dirty = false;
                if let Some(handle) = locked.flush_handle.take() {
                    handle.abort();
                }
                info!("Flushed annotations to {}", locked.file_path.display());
            }
        }
        Ok(())
    }

    /// Flush ALL dirty documents to disk. Called on shutdown.
    pub async fn flush_all(&self) -> Result<(), DocError> {
        let docs = self.docs.read().await;
        for (path, state) in docs.iter() {
            let mut locked = state.lock().await;
            if locked.dirty {
                let anns = read_doc(&locked.doc);
                if let Err(e) = store::save_anns(&locked.file_path, &anns) {
                    error!("Failed to flush {}: {}", path, e);
                } else {
                    locked.dirty = false;
                    if let Some(handle) = locked.flush_handle.take() {
                        handle.abort();
                    }
                    info!("Flushed annotations to {}", locked.file_path.display());
                }
            }
        }
        Ok(())
    }

    fn schedule_flush(&self, doc_state: &mut DocState, file_path: String) {
        // Cancel existing timer
        if let Some(handle) = doc_state.flush_handle.take() {
            handle.abort();
        }
        doc_state.dirty = true;

        let docs = self.docs.clone();
        doc_state.flush_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(FLUSH_DEBOUNCE).await;
            let docs = docs.read().await;
            if let Some(state) = docs.get(&file_path) {
                let mut locked = state.lock().await;
                if locked.dirty {
                    let anns = read_doc(&locked.doc);
                    if let Err(e) = store::save_anns(&locked.file_path, &anns) {
                        error!("Failed to flush {}: {}", file_path, e);
                    } else {
                        locked.dirty = false;
                        info!(
                            "Debounced flush: saved annotations to {}",
                            locked.file_path.display()
                        );
                    }
                }
            }
        }));
    }
}

/// Populate a YJS Doc from a Vec<Ann>.
fn populate_doc(doc: &Doc, anns: &[Ann]) {
    let mut txn = doc.transact_mut();
    let annotations = txn.get_or_insert_map("annotations");

    for ann in anns {
        let ann_map: yrs::MapRef = annotations.insert(&mut txn, ann.id.as_str(), MapPrelim::default());

        // Selector sub-map
        let selector: yrs::MapRef =
            ann_map.insert(&mut txn, "selector", MapPrelim::default());
        selector.insert(&mut txn, "quote", ann.selector.quote.clone());
        selector.insert(&mut txn, "prefix", ann.selector.prefix.clone());
        selector.insert(&mut txn, "suffix", ann.selector.suffix.clone());

        // Thread array
        let thread: yrs::ArrayRef = ann_map.insert(
            &mut txn,
            "thread",
            yrs::ArrayPrelim::default(),
        );
        for comment in &ann.thread {
            let comment_map = MapPrelim::from([
                ("id", comment.id.as_str()),
                ("author", comment.author.as_str()),
                ("created", comment.created.as_str()),
                ("body", comment.body.as_str()),
            ]);
            thread.push_back(&mut txn, comment_map);
        }
    }
}

/// Read a YJS Doc back into Vec<Ann>.
fn read_doc(doc: &Doc) -> Vec<Ann> {
    let txn = doc.transact();
    let annotations = match txn.get_map("annotations") {
        Some(map) => map,
        None => return Vec::new(),
    };

    let mut anns = Vec::new();

    for (id, value) in annotations.iter(&txn) {
        let ann_map = match value {
            Out::YMap(m) => m,
            _ => continue,
        };

        // Read selector
        let selector = match ann_map.get(&txn, "selector") {
            Some(Out::YMap(sel_map)) => {
                let quote = get_map_str(&sel_map, &txn, "quote");
                let prefix = get_map_str(&sel_map, &txn, "prefix");
                let suffix = get_map_str(&sel_map, &txn, "suffix");
                Selector {
                    quote,
                    prefix,
                    suffix,
                }
            }
            _ => Selector {
                quote: String::new(),
                prefix: String::new(),
                suffix: String::new(),
            },
        };

        // Read thread
        let mut thread = Vec::new();
        if let Some(Out::YArray(thread_arr)) = ann_map.get(&txn, "thread") {
            for item in thread_arr.iter(&txn) {
                if let Out::YMap(comment_map) = item {
                    thread.push(Comment {
                        id: get_map_str(&comment_map, &txn, "id"),
                        author: get_map_str(&comment_map, &txn, "author"),
                        created: get_map_str(&comment_map, &txn, "created"),
                        body: get_map_str(&comment_map, &txn, "body"),
                    });
                }
            }
        }

        anns.push(Ann {
            id: id.to_string(),
            selector,
            thread,
        });
    }

    // Sort by ID for deterministic output
    anns.sort_by(|a, b| a.id.cmp(&b.id));
    anns
}

fn get_map_str(map: &yrs::MapRef, txn: &yrs::Transaction, key: &str) -> String {
    match map.get(txn, key) {
        Some(Out::Any(yrs::Any::String(s))) => s.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use yrs::updates::encoder::Encode;

    fn test_anns() -> Vec<Ann> {
        vec![
            Ann {
                id: "k8f2a".into(),
                selector: Selector {
                    quote: "ESP32-S3".into(),
                    prefix: "architecture of the ".into(),
                    suffix: " microcontroller".into(),
                },
                thread: vec![
                    Comment {
                        id: "c001".into(),
                        author: "njr".into(),
                        created: "2026-02-10T14:30:00Z".into(),
                        body: "Should we verify the power draw?".into(),
                    },
                    Comment {
                        id: "c002".into(),
                        author: "alice".into(),
                        created: "2026-02-10T15:12:00Z".into(),
                        body: "Yes - measured at 45uA.".into(),
                    },
                ],
            },
            Ann {
                id: "m3x9p".into(),
                selector: Selector {
                    quote: "userspace packet loop".into(),
                    prefix: "Implement ".into(),
                    suffix: " in src-tauri".into(),
                },
                thread: vec![Comment {
                    id: "c003".into(),
                    author: "njr".into(),
                    created: "2026-02-11T09:00:00Z".into(),
                    body: "This is done.".into(),
                }],
            },
        ]
    }

    #[test]
    fn test_populate_and_read_roundtrip() {
        let anns = test_anns();
        let doc = Doc::new();
        populate_doc(&doc, &anns);
        let result = read_doc(&doc);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "k8f2a");
        assert_eq!(result[0].selector.quote, "ESP32-S3");
        assert_eq!(result[0].thread.len(), 2);
        assert_eq!(result[0].thread[0].id, "c001");
        assert_eq!(result[0].thread[0].author, "njr");
        assert_eq!(result[0].thread[1].id, "c002");

        assert_eq!(result[1].id, "m3x9p");
        assert_eq!(result[1].thread.len(), 1);
    }

    #[test]
    fn test_empty_doc_reads_empty() {
        let doc = Doc::new();
        let result = read_doc(&doc);
        assert!(result.is_empty());
    }

    #[test]
    fn test_populate_empty_anns() {
        let doc = Doc::new();
        populate_doc(&doc, &[]);
        let result = read_doc(&doc);
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_or_create_loads_from_disk() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("README.md");
        let ann_file = dir.path().join("README.ann.kdl");

        let anns = test_anns();
        store::save_anns(&ann_file, &anns).unwrap();

        let manager = DocManager::new();
        let result = manager
            .read_annotations(file.to_string_lossy().as_ref())
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "k8f2a");
    }

    #[tokio::test]
    async fn test_get_or_create_missing_file() {
        let manager = DocManager::new();
        let result = manager
            .read_annotations("/nonexistent/README.md")
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_state_vector_and_diff() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("README.md");
        let ann_file = dir.path().join("README.ann.kdl");

        let anns = test_anns();
        store::save_anns(&ann_file, &anns).unwrap();

        let manager = DocManager::new();
        let file_path = file.to_string_lossy().to_string();

        // Get state vector from the populated doc
        let sv = manager.state_vector(&file_path).await.unwrap();
        assert!(!sv.is_empty());

        // Empty state vector should produce a full diff
        let empty_sv = yrs::StateVector::default().encode_v1();
        let diff = manager.encode_diff(&file_path, &empty_sv).await.unwrap();
        assert!(!diff.is_empty());

        // Apply diff to a new doc and verify
        let doc2 = Doc::new();
        {
            let mut txn = doc2.transact_mut();
            let update = yrs::Update::decode_v1(&diff).unwrap();
            txn.apply_update(update).unwrap();
        }
        let result = read_doc(&doc2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "k8f2a");
    }

    #[tokio::test]
    async fn test_apply_update_broadcasts() {
        let manager = DocManager::new();
        let file_path = "/tmp/test_broadcast.md";

        // Subscribe before applying update
        let mut rx = manager.subscribe(file_path).await;

        // Create an update from a separate doc
        let doc = Doc::new();
        let update = {
            let mut txn = doc.transact_mut();
            let map = txn.get_or_insert_map("annotations");
            let ann: yrs::MapRef = map.insert(&mut txn, "test1", MapPrelim::default());
            let sel: yrs::MapRef =
                ann.insert(&mut txn, "selector", MapPrelim::default());
            sel.insert(&mut txn, "quote", "hello");

            let sv = yrs::StateVector::default().encode_v1();
            txn.encode_state_as_update_v1(
                &yrs::StateVector::decode_v1(&sv).unwrap(),
            )
        };

        manager.apply_update(file_path, &update).await.unwrap();

        // Should have received the broadcast
        let received = rx.try_recv().unwrap();
        assert_eq!(received, update);
    }

    #[tokio::test]
    async fn test_concurrent_edits() {
        let manager = DocManager::new();
        let file_path = "/tmp/test_concurrent.md";

        // First update: add annotation "a1"
        let doc1 = Doc::new();
        let update1 = {
            let mut txn = doc1.transact_mut();
            let map = txn.get_or_insert_map("annotations");
            let ann: yrs::MapRef = map.insert(&mut txn, "a1", MapPrelim::default());
            let sel: yrs::MapRef =
                ann.insert(&mut txn, "selector", MapPrelim::default());
            sel.insert(&mut txn, "quote", "first");
            let sv = yrs::StateVector::default().encode_v1();
            txn.encode_state_as_update_v1(
                &yrs::StateVector::decode_v1(&sv).unwrap(),
            )
        };

        manager.apply_update(file_path, &update1).await.unwrap();

        // Second update: add annotation "a2" (from a different client)
        let doc2 = Doc::new();
        let update2 = {
            let mut txn = doc2.transact_mut();
            let map = txn.get_or_insert_map("annotations");
            let ann: yrs::MapRef = map.insert(&mut txn, "a2", MapPrelim::default());
            let sel: yrs::MapRef =
                ann.insert(&mut txn, "selector", MapPrelim::default());
            sel.insert(&mut txn, "quote", "second");
            let sv = yrs::StateVector::default().encode_v1();
            txn.encode_state_as_update_v1(
                &yrs::StateVector::decode_v1(&sv).unwrap(),
            )
        };

        manager.apply_update(file_path, &update2).await.unwrap();

        // Both should be present
        let result = manager.read_annotations(file_path).await.unwrap();
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"a1"));
        assert!(ids.contains(&"a2"));
    }

    #[tokio::test]
    async fn test_flush_writes_to_disk() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("README.md");
        let ann_file = dir.path().join("README.ann.kdl");

        let manager = DocManager::new();
        let file_path = file.to_string_lossy().to_string();

        // Create doc and add an annotation via direct populate
        {
            let state = manager.get_or_create(&file_path).await;
            let mut locked = state.lock().await;
            let anns = vec![Ann {
                id: "flush-test".into(),
                selector: Selector {
                    quote: "test text".into(),
                    prefix: "".into(),
                    suffix: "".into(),
                },
                thread: vec![],
            }];
            populate_doc(&locked.doc, &anns);
            locked.dirty = true;
        }

        // File should not exist yet (no flush)
        assert!(!ann_file.exists());

        // Flush
        manager.flush(&file_path).await.unwrap();

        // File should now exist with data
        assert!(ann_file.exists());
        let loaded = store::load_anns(&ann_file).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "flush-test");
    }
}
