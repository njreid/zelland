use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const TTL: Duration = Duration::from_secs(30 * 60); // 30 minutes
const CLEANUP_INTERVAL: Duration = Duration::from_secs(5 * 60); // 5 minutes

struct AssetEntry {
    file_path: PathBuf,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct AssetManager {
    assets: Arc<RwLock<HashMap<String, AssetEntry>>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the background cleanup loop. Call once at startup.
    pub fn start_cleanup(&self) {
        let assets = self.assets.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                let mut map = assets.write().await;
                let now = Instant::now();
                map.retain(|_, entry| entry.expires_at > now);
            }
        });
    }

    /// Register a file and return its asset ID. Verifies file exists.
    pub async fn register(&self, file_path: &Path) -> Result<String, std::io::Error> {
        let abs_path = std::fs::canonicalize(file_path)?;
        // Verify file exists (canonicalize already does this, but be explicit)
        std::fs::metadata(&abs_path)?;

        let id = generate_id();
        let entry = AssetEntry {
            file_path: abs_path,
            expires_at: Instant::now() + TTL,
        };

        self.assets.write().await.insert(id.clone(), entry);
        Ok(id)
    }

    /// Resolve an asset ID to a file path. Returns None if not found or expired.
    pub async fn resolve(&self, id: &str) -> Option<PathBuf> {
        let map = self.assets.read().await;
        let entry = map.get(id)?;
        if Instant::now() > entry.expires_at {
            return None;
        }
        Some(entry.file_path.clone())
    }
}

fn generate_id() -> String {
    let mut buf = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut buf);
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello").unwrap();

        let mgr = AssetManager::new();
        let id = mgr.register(&file).await.unwrap();
        assert_eq!(id.len(), 16); // 8 bytes -> 16 hex chars

        let resolved = mgr.resolve(&id).await;
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), std::fs::canonicalize(&file).unwrap());
    }

    #[tokio::test]
    async fn test_resolve_unknown_id() {
        let mgr = AssetManager::new();
        assert!(mgr.resolve("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_register_nonexistent_file() {
        let mgr = AssetManager::new();
        let result = mgr.register(Path::new("/nonexistent/file.txt")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_id_uniqueness() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[tokio::test]
    async fn test_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "data").unwrap();

        let mgr = AssetManager::new();

        // Manually insert with past expiry
        {
            let mut map = mgr.assets.write().await;
            map.insert(
                "expired-id".to_string(),
                AssetEntry {
                    file_path: file.clone(),
                    expires_at: Instant::now() - Duration::from_secs(1),
                },
            );
        }

        // Should not resolve
        assert!(mgr.resolve("expired-id").await.is_none());
    }
}
