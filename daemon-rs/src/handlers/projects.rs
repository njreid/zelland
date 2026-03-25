use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::projects::{scan_projects, Project};
use crate::server::AppState;
use crate::watcher::WatchCommand;

pub async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<Project>>, (StatusCode, String)> {
    let projects = scan_projects(&state.config.projects_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read projects directory: {}", e),
        )
    })?;
    Ok(Json(projects))
}

#[derive(serde::Deserialize)]
pub struct ActivateRequest {
    pub project_id: String,
}

pub async fn activate_project(
    State(state): State<AppState>,
    Json(req): Json<ActivateRequest>,
) -> StatusCode {
    let project_path = state.config.projects_path.join(&req.project_id);
    if project_path.exists() && project_path.is_dir() {
        info!("Activating project: {}", req.project_id);
        let _ = state.watcher_tx.send(WatchCommand::AddRecursive(project_path)).await;
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

#[derive(serde::Serialize)]
pub struct ProjectFiles {
    pub root_md: Vec<String>,
    pub docs_md: Vec<String>,
}

fn collect_md_recursive(dir: &std::path::Path, project_root: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        if path.is_dir() {
            collect_md_recursive(&path, project_root, out);
        } else if name.ends_with(".md") {
            if let Ok(rel) = path.strip_prefix(project_root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn collect_md_recursive_finds_nested_md() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        write(&root.join("README.md"), "# hi");
        write(&root.join("sub/guide.md"), "guide");
        write(&root.join("sub/deep/ref.md"), "ref");
        write(&root.join("sub/notes.txt"), "ignore me");
        write(&root.join(".hidden.md"), "hidden");

        let mut out = vec![];
        collect_md_recursive(root, root, &mut out);
        out.sort();

        assert_eq!(out, vec!["README.md", "sub/deep/ref.md", "sub/guide.md"]);
    }

    #[test]
    fn collect_md_recursive_skips_dotfiles() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        write(&root.join(".hidden/secret.md"), "secret");
        write(&root.join("visible.md"), "ok");

        let mut out = vec![];
        collect_md_recursive(root, root, &mut out);

        assert_eq!(out, vec!["visible.md"]);
    }

    #[test]
    fn collect_md_recursive_empty_dir() {
        let dir = tempdir().unwrap();
        let mut out = vec![];
        collect_md_recursive(dir.path(), dir.path(), &mut out);
        assert!(out.is_empty());
    }

    fn make_state(projects_path: std::path::PathBuf) -> crate::server::AppState {
        use std::sync::Arc;
        use tokio::sync::mpsc;
        use crate::assets::AssetManager;
        use crate::config::Config;
        use crate::loro_manager::LoroManager;
        use crate::server::AppState;
        use crate::ws::ClientRegistry;

        let config = Config {
            port: 0,
            cert_file: None,
            key_file: None,
            projects_path,
        };
        let (watcher_tx, _rx) = mpsc::channel(16);
        AppState {
            config: Arc::new(config),
            asset_manager: AssetManager::new(),
            registry: ClientRegistry::new(),
            watcher_tx,
            loro_manager: Arc::new(LoroManager::new()),
        }
    }

    #[tokio::test]
    async fn list_project_files_returns_root_and_docs() {
        use axum::{body::Body, http::{Request, StatusCode}, Router};
        use tower::ServiceExt;

        let projects_dir = tempdir().unwrap();
        let proj = projects_dir.path().join("myproject");
        write(&proj.join("README.md"), "# hi");
        write(&proj.join("PLAN.md"), "plan");
        write(&proj.join("src/main.rs"), "fn main() {}"); // non-md, ignored
        write(&proj.join("docs/guide.md"), "guide");
        write(&proj.join("docs/sub/ref.md"), "ref");

        let state = make_state(projects_dir.path().to_path_buf());
        let app = Router::new()
            .route("/api/v1/projects/{id}/files", axum::routing::get(list_project_files))
            .with_state(state);

        let resp = app
            .oneshot(Request::builder().uri("/api/v1/projects/myproject/files").body(Body::empty()).unwrap())
            .await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let files: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let mut root_md: Vec<&str> = files["root_md"].as_array().unwrap()
            .iter().map(|v| v.as_str().unwrap()).collect();
        root_md.sort();
        assert_eq!(root_md, vec!["PLAN.md", "README.md"]);

        let mut docs_md: Vec<&str> = files["docs_md"].as_array().unwrap()
            .iter().map(|v| v.as_str().unwrap()).collect();
        docs_md.sort();
        assert_eq!(docs_md, vec!["docs/guide.md", "docs/sub/ref.md"]);
    }

    #[tokio::test]
    async fn list_project_files_returns_404_for_unknown_project() {
        use axum::{body::Body, http::{Request, StatusCode}, Router};
        use tower::ServiceExt;

        let projects_dir = tempdir().unwrap();
        let state = make_state(projects_dir.path().to_path_buf());
        let app = Router::new()
            .route("/api/v1/projects/{id}/files", axum::routing::get(list_project_files))
            .with_state(state);

        let resp = app
            .oneshot(Request::builder().uri("/api/v1/projects/nonexistent/files").body(Body::empty()).unwrap())
            .await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

pub async fn list_project_files(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
) -> Result<Json<ProjectFiles>, (StatusCode, String)> {
    let project_path = state.config.projects_path.join(&project_id);
    if !project_path.is_dir() {
        return Err((StatusCode::NOT_FOUND, format!("Project '{}' not found", project_id)));
    }

    // Root-level .md files only
    let mut root_md: Vec<String> = std::fs::read_dir(&project_path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let ft = e.file_type().ok()?;
            if !ft.is_file() { return None; }
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && !name.starts_with('.') { Some(name) } else { None }
        })
        .collect();
    root_md.sort();

    // docs/** .md files (recursive)
    let mut docs_md: Vec<String> = Vec::new();
    let docs_path = project_path.join("docs");
    if docs_path.is_dir() {
        collect_md_recursive(&docs_path, &project_path, &mut docs_md);
        docs_md.sort();
    }

    Ok(Json(ProjectFiles { root_md, docs_md }))
}
