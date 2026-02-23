use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::assets::AssetManager;
use crate::config::Config;
use crate::handlers;
use crate::watcher::{self, WatchCommand};
use crate::ws::{self, ClientRegistry};
use crate::loro_manager::LoroManager;

/// Shared application state, passed to all handlers via axum's State extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub asset_manager: AssetManager,
    pub registry: ClientRegistry,
    pub watcher_tx: mpsc::Sender<WatchCommand>,
    pub loro_manager: Arc<LoroManager>,
}

/// Build the axum router with all routes.
pub fn build_router(state: AppState) -> Router {
    // Trigger routes (loopback-only)
    let trigger_routes = Router::new()
        .route("/show", post(handlers::trigger::trigger_show))
        .route("/md", post(handlers::trigger::trigger_md))
        .route("/notify", post(handlers::trigger::trigger_notify))
        .layer(middleware::from_fn(loopback_guard));

    Router::new()
        // API routes
        .route("/api/v1/projects", get(handlers::projects::list_projects))
        .route(
            "/api/v1/projects/activate",
            post(handlers::projects::activate_project),
        )
        .route("/api/v1/fs/read", get(handlers::fs::read_file))
        .route("/api/v1/fs/annotate", post(handlers::fs::annotate_file))
        .route(
            "/api/v1/sessions/recent",
            get(handlers::sessions::get_recent_sessions)
                .post(handlers::sessions::record_session),
        )
        // Trigger routes (loopback-only)
        .nest("/api/v1/trigger", trigger_routes)
        // Asset serving
        .route("/assets/{id}", get(handlers::assets::serve_asset))
        // WebSocket
        .route("/ws", get(ws_upgrade))
        // Annotation endpoints
        .route(
            "/annotations/sync/{*filepath}",
            get(handlers::annotations::annotation_sync),
        )
        .route(
            "/annotations/{*filepath}",
            get(handlers::annotations::get_annotations)
                .put(handlers::annotations::put_annotations),
        )
        .with_state(state)
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        ws::handle_ws(socket, state.registry, state.asset_manager)
    })
}

/// Middleware that rejects non-loopback connections.
async fn loopback_guard(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if !addr.ip().is_loopback() {
        warn!(
            "Blocked non-loopback access to {} from {}",
            request.uri().path(),
            addr
        );
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    next.run(request).await
}

/// Start the server. Call after building AppState.
pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let asset_manager = AssetManager::new();
    asset_manager.start_cleanup();

    let registry = ClientRegistry::new();
    let loro_manager = Arc::new(LoroManager::new());

    let watcher_tx = watcher::start_watcher(registry.clone(), asset_manager.clone());

    let state = AppState {
        config: Arc::new(config.clone()),
        asset_manager,
        registry,
        watcher_tx,
        loro_manager,
    };

    let app = build_router(state).into_make_service_with_connect_info::<SocketAddr>();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    // Graceful shutdown
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Shutdown signal received, exiting...");
            std::process::exit(0);
        }
    });

    info!("zlnd listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            port: 0,
            cert_file: None,
            key_file: None,
            #[allow(deprecated)]
            projects_path: dir.into_path(),
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
    async fn test_projects_endpoint() {
        let state = test_state();
        // Create some project dirs
        let projects_path = state.config.projects_path.clone();
        std::fs::create_dir(projects_path.join("alpha")).unwrap();
        std::fs::create_dir(projects_path.join("beta")).unwrap();

        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let projects: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0]["id"], "alpha");
        assert_eq!(projects[1]["id"], "beta");
    }

    #[tokio::test]
    async fn test_activate_endpoint() {
        let state = test_state();
        let app = build_router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects/activate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"project_id":"test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_fs_read_endpoint() {
        let state = test_state();
        let file = state.config.projects_path.join("hello.txt");
        std::fs::write(&file, "hello world").unwrap();

        let app = build_router(state);
        let uri = "/api/v1/fs/read?path=hello.txt";

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"hello world");
    }

    #[tokio::test]
    async fn test_fs_read_missing_path() {
        let state = test_state();
        let app = build_router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/fs/read?path=/nonexistent/file.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_asset_not_found() {
        let state = test_state();
        let app = build_router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/assets/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_asset_register_and_serve() {
        let state = test_state();
        let file = state.config.projects_path.join("test.png");
        std::fs::write(&file, b"\x89PNG fake").unwrap();

        let id = state.asset_manager.register(&file).await.unwrap();

        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/assets/{}", id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"\x89PNG fake");
    }
}
