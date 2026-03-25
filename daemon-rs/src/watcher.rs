use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::assets::AssetManager;
use crate::proto::zelland::{self, envelope::Payload, Envelope, OpenViewRequest};
use crate::ws::ClientRegistry;

/// Commands to control the file watcher.
pub enum WatchCommand {
    Add(PathBuf),
    AddRecursive(PathBuf),
}

/// Detect file type from extension, matching the Go daemon's logic.
pub fn detect_file_type(path: &Path) -> zelland::open_view_request::FileType {
    match path.extension().and_then(|e| e.to_str()) {
        Some("md") => zelland::open_view_request::FileType::Markdown,
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp") | Some("svg") => {
            zelland::open_view_request::FileType::Image
        }
        Some("pdf") => zelland::open_view_request::FileType::Pdf,
        _ => zelland::open_view_request::FileType::Unknown,
    }
}

use std::sync::Arc;
use crate::loro_manager::LoroManager;

/// Start the file watcher loop. Returns a sender for WatchCommand.
pub fn start_watcher(
    registry: ClientRegistry,
    asset_manager: AssetManager,
    loro_manager: Arc<LoroManager>,
) -> mpsc::Sender<WatchCommand> {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<WatchCommand>(64);

    tokio::spawn(async move {
        let (event_tx, mut event_rx) = mpsc::channel(256);

        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = event_tx.blocking_send(event);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                error!("Failed to create file watcher: {}", e);
                return;
            }
        };

        info!("File watcher started");

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        WatchCommand::Add(path) => {
                            if let Err(e) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                                warn!("Failed to watch {}: {}", path.display(), e);
                            } else {
                                info!("Watching: {}", path.display());
                            }
                        }
                        WatchCommand::AddRecursive(path) => {
                            if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                                warn!("Failed to watch recursive {}: {}", path.display(), e);
                            } else {
                                info!("Watching recursive: {}", path.display());
                            }
                        }
                    }
                }
                Some(event) = event_rx.recv() => {
                    handle_event(&event, &registry, &asset_manager, &loro_manager).await;
                }
                else => break,
            }
        }
    });

    cmd_tx
}

async fn handle_event(
    event: &Event,
    registry: &ClientRegistry,
    asset_manager: &AssetManager,
    loro_manager: &LoroManager,
) {
    if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
        return;
    }

    for path in &event.paths {
        // Skip hidden files/dirs (like .zelland cache)
        if path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
            continue;
        }

        let ftype = detect_file_type(path);
        // We are primarily interested in Markdown updates for live preview/annotations
        if ftype == zelland::open_view_request::FileType::Unknown {
            continue;
        }

        // If it's a markdown file, also trigger a Loro reload to pick up annotation changes
        if ftype == zelland::open_view_request::FileType::Markdown {
            loro_manager.reload_from_disk(path).await;
        }

        // Reverse lookup: find asset_id for this path
        let mut asset_id = None;
        {
            let map = asset_manager.assets.read().await;
            for (id, entry) in map.iter() {
                if entry.file_path == *path {
                    asset_id = Some(id.clone());
                    break;
                }
            }
        }

        // If no asset ID found, but it's a markdown file, we might still want to notify.
        // However, the client needs an ID to fetch it.
        // For simplicity, we'll auto-register it if it doesn't exist yet but was modified.
        let asset_id = match asset_id {
            Some(id) => id,
            None => {
                if ftype == zelland::open_view_request::FileType::Markdown {
                    match asset_manager.register(path).await {
                        Ok(id) => id,
                        Err(_) => continue,
                    }
                } else {
                    continue;
                }
            }
        };

        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let timestamp = chrono::Utc::now().timestamp();
        let envelope = Envelope {
            payload: Some(Payload::OpenView(OpenViewRequest {
                asset_id: asset_id.clone(),
                url: format!("/assets/{}?t={}", asset_id, timestamp),
                file_type: ftype as i32,
                title,
            })),
        };

        registry.broadcast(&envelope);
        info!("Broadcast file update for {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_type_markdown() {
        assert_eq!(
            detect_file_type(Path::new("README.md")),
            zelland::open_view_request::FileType::Markdown
        );
    }

    #[test]
    fn test_detect_file_type_images() {
        assert_eq!(
            detect_file_type(Path::new("photo.png")),
            zelland::open_view_request::FileType::Image
        );
        assert_eq!(
            detect_file_type(Path::new("photo.jpg")),
            zelland::open_view_request::FileType::Image
        );
        assert_eq!(
            detect_file_type(Path::new("photo.jpeg")),
            zelland::open_view_request::FileType::Image
        );
        assert_eq!(
            detect_file_type(Path::new("icon.svg")),
            zelland::open_view_request::FileType::Image
        );
    }

    #[test]
    fn test_detect_file_type_pdf() {
        assert_eq!(
            detect_file_type(Path::new("doc.pdf")),
            zelland::open_view_request::FileType::Pdf
        );
    }

    #[test]
    fn test_detect_file_type_unknown() {
        assert_eq!(
            detect_file_type(Path::new("data.csv")),
            zelland::open_view_request::FileType::Unknown
        );
        assert_eq!(
            detect_file_type(Path::new("noext")),
            zelland::open_view_request::FileType::Unknown
        );
    }

    #[tokio::test]
    async fn test_watcher_init() {
        let registry = ClientRegistry::new();
        let asset_manager = AssetManager::new();
        let loro_manager = Arc::new(LoroManager::new());
        let _tx = start_watcher(registry, asset_manager, loro_manager);
    }
}
