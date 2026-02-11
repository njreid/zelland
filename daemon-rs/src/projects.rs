use serde::{Deserialize, Serialize};
use std::path::Path;

/// Project as returned by the daemon's REST API.
/// Must match the JSON shape expected by the Tauri client (`daemon.rs`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: Option<String>,
    pub host: String,
    pub session_name: String,
    pub root_path: String,
}

/// Scan a directory for project folders.
/// Each immediate subdirectory becomes a project. Hidden directories are skipped.
pub fn scan_projects(projects_path: &Path) -> Result<Vec<Project>, std::io::Error> {
    let entries = std::fs::read_dir(projects_path)?;
    let mut projects = Vec::new();

    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        projects.push(Project {
            id: name.clone(),
            name: Some(name.clone()),
            host: "localhost".to_string(),
            session_name: name.clone(),
            root_path: entry.path().to_string_lossy().to_string(),
        });
    }

    // Sort by name for deterministic output
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scan_empty_dir() {
        let dir = tempdir().unwrap();
        let projects = scan_projects(dir.path()).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_scan_projects() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("alpha")).unwrap();
        std::fs::create_dir(dir.path().join("beta")).unwrap();
        // Create a file (should be skipped)
        std::fs::write(dir.path().join("readme.txt"), "hi").unwrap();

        let projects = scan_projects(dir.path()).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, "alpha");
        assert_eq!(projects[1].id, "beta");
        assert_eq!(projects[0].session_name, "alpha");
        assert_eq!(projects[0].host, "localhost");
    }

    #[test]
    fn test_scan_skips_hidden_dirs() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("visible")).unwrap();
        std::fs::create_dir(dir.path().join(".hidden")).unwrap();

        let projects = scan_projects(dir.path()).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "visible");
    }

    #[test]
    fn test_scan_nonexistent_dir() {
        let result = scan_projects(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_project_json_shape() {
        let project = Project {
            id: "myproject".into(),
            name: Some("myproject".into()),
            host: "localhost".into(),
            session_name: "myproject".into(),
            root_path: "/home/user/code/myproject".into(),
        };
        let json = serde_json::to_value(&project).unwrap();
        assert_eq!(json["id"], "myproject");
        assert_eq!(json["name"], "myproject");
        assert_eq!(json["host"], "localhost");
        assert_eq!(json["session_name"], "myproject");
        assert_eq!(json["root_path"], "/home/user/code/myproject");
    }
}
