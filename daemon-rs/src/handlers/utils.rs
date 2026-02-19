use crate::server::AppState;

/// Resolve a URL filepath to an absolute filesystem path.
pub fn resolve_filepath(state: &AppState, filepath: &str) -> String {
    let filepath = filepath.strip_prefix('/').unwrap_or(filepath);
    state
        .config
        .projects_path
        .join(filepath)
        .to_string_lossy()
        .to_string()
}
