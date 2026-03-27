use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub port: u16,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub projects_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let projects_path = dirs_home()
            .map(|h| h.join("code"))
            .unwrap_or_default();

        Config {
            port: 0,   // 0 = let OS assign a free port
            cert_file: None,
            key_file: None,
            projects_path,
        }
    }
}

impl Config {
    /// Load config from a KDL file, falling back to defaults for missing fields.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(e, path.to_path_buf()))?;
        Self::parse(&content)
    }

    /// Parse config from a KDL string.
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        let doc: kdl::KdlDocument = input.parse().map_err(ConfigError::Kdl)?;
        let defaults = Config::default();

        let port = get_i64(&doc, "port")
            .map(|v| v as u16)
            .unwrap_or(defaults.port);

        let cert_file = get_string(&doc, "cert_file");
        let key_file = get_string(&doc, "key_file");

        let projects_path = get_string(&doc, "projects_path")
            .map(PathBuf::from)
            .unwrap_or(defaults.projects_path);

        Ok(Config {
            port,
            cert_file,
            key_file,
            projects_path,
        })
    }

    /// Merge CLI overrides into this config. `None` values are ignored.
    pub fn merge(&mut self, port: Option<u16>, projects_path: Option<PathBuf>) {
        if let Some(p) = port {
            self.port = p;
        }
        if let Some(pp) = projects_path {
            self.projects_path = pp;
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error, PathBuf),
    Kdl(kdl::KdlError),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e, path) => write!(f, "failed to read config {}: {}", path.display(), e),
            ConfigError::Kdl(e) => write!(f, "KDL parse error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

fn get_string(doc: &kdl::KdlDocument, name: &str) -> Option<String> {
    doc.get(name)?
        .entries()
        .first()?
        .value()
        .as_string()
        .map(|s| s.to_string())
}

fn get_i64(doc: &kdl::KdlDocument, name: &str) -> Option<i64> {
    doc.get(name)?
        .entries()
        .first()?
        .value()
        .as_integer()
        .map(|v| v as i64)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.port, 0);
        assert!(cfg.cert_file.is_none());
        assert!(cfg.key_file.is_none());
        // projects_path should end with "code"
        assert!(cfg.projects_path.ends_with("code"));
    }

    #[test]
    fn test_parse_full_config() {
        let input = r#"
port 9090
cert_file "/etc/ssl/cert.pem"
key_file "/etc/ssl/key.pem"
projects_path "/srv/projects"
"#;
        let cfg = Config::parse(input).unwrap();
        assert_eq!(cfg.port, 9090);
        assert_eq!(cfg.cert_file.as_deref(), Some("/etc/ssl/cert.pem"));
        assert_eq!(cfg.key_file.as_deref(), Some("/etc/ssl/key.pem"));
        assert_eq!(cfg.projects_path, PathBuf::from("/srv/projects"));
    }

    #[test]
    fn test_parse_partial_config_uses_defaults() {
        let input = "port 4000\n";
        let cfg = Config::parse(input).unwrap();
        assert_eq!(cfg.port, 4000);
        assert!(cfg.cert_file.is_none());
        assert!(cfg.key_file.is_none());
        // projects_path falls back to default ~/code
        assert!(cfg.projects_path.ends_with("code"));
    }

    #[test]
    fn test_parse_empty_config_uses_defaults() {
        let cfg = Config::parse("").unwrap();
        assert_eq!(cfg.port, 0);
    }

    #[test]
    fn test_merge_overrides() {
        let mut cfg = Config::default();
        cfg.merge(Some(9999), Some(PathBuf::from("/tmp/projects")));
        assert_eq!(cfg.port, 9999);
        assert_eq!(cfg.projects_path, PathBuf::from("/tmp/projects"));
    }

    #[test]
    fn test_merge_none_preserves() {
        let mut cfg = Config::default();
        let original_port = cfg.port;
        cfg.merge(None, None);
        assert_eq!(cfg.port, original_port);
    }

    #[test]
    fn test_load_missing_file() {
        let result = Config::load(Path::new("/nonexistent/config.kdl"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failed to read config"));
    }

    #[test]
    fn test_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.kdl");
        std::fs::write(&path, "port 7777\nprojects_path \"/my/projects\"\n").unwrap();

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.port, 7777);
        assert_eq!(cfg.projects_path, PathBuf::from("/my/projects"));
    }

    #[test]
    fn test_parse_invalid_kdl() {
        let result = Config::parse("this is not { valid kdl");
        assert!(result.is_err());
    }
}
