use std::path::Path;

/// An annotation stored in a KDL sidecar file.
/// Matches the Go `kdl.Annotation` struct for wire compatibility.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub user: String,
    pub timestamp: i64,
    pub context_hash: String,
    pub target_text: String,
    pub body: String,
}

/// Load annotations from a KDL file. Returns empty vec if file doesn't exist.
pub fn load_annotations(path: &Path) -> Result<Vec<Annotation>, StoreError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Io(e)),
    };

    parse_annotations(&content)
}

/// Save annotations to a KDL file, overwriting any existing content.
pub fn save_annotations(path: &Path, annotations: &[Annotation]) -> Result<(), StoreError> {
    let content = serialize_annotations(annotations);
    std::fs::write(path, content).map_err(StoreError::Io)
}

/// Append (upsert) an annotation: update if ID exists, otherwise add to end.
pub fn append_annotation(path: &Path, ann: Annotation) -> Result<(), StoreError> {
    let mut anns = load_annotations(path)?;

    let mut found = false;
    for existing in &mut anns {
        if existing.id == ann.id {
            *existing = ann.clone();
            found = true;
            break;
        }
    }
    if !found {
        anns.push(ann);
    }

    save_annotations(path, &anns)
}

fn parse_annotations(input: &str) -> Result<Vec<Annotation>, StoreError> {
    let doc: kdl::KdlDocument = input.parse().map_err(StoreError::Kdl)?;
    let mut annotations = Vec::new();

    for node in doc.nodes() {
        if node.name().value() != "annotation" {
            continue;
        }

        let id = get_prop(node, "id").unwrap_or_default();
        let user = get_prop(node, "user").unwrap_or_default();
        let timestamp = get_prop_i64(node, "timestamp").unwrap_or(0);

        // Children are child nodes within the annotation block
        let children = node.children();
        let context_hash = children
            .and_then(|c| get_child_value(c, "context_hash"))
            .unwrap_or_default();
        let target_text = children
            .and_then(|c| get_child_value(c, "target_text"))
            .unwrap_or_default();
        let body = children
            .and_then(|c| get_child_value(c, "body"))
            .unwrap_or_default();

        annotations.push(Annotation {
            id,
            user,
            timestamp,
            context_hash,
            target_text,
            body,
        });
    }

    Ok(annotations)
}

fn serialize_annotations(annotations: &[Annotation]) -> String {
    let mut out = String::new();
    for ann in annotations {
        out.push_str(&format!(
            "annotation id={} user={} timestamp={} {{\n",
            kdl_quote(&ann.id),
            kdl_quote(&ann.user),
            ann.timestamp,
        ));
        out.push_str(&format!("    context_hash {}\n", kdl_quote(&ann.context_hash)));
        out.push_str(&format!("    target_text {}\n", kdl_quote(&ann.target_text)));
        out.push_str(&format!("    body {}\n", kdl_quote(&ann.body)));
        out.push_str("}\n");
    }
    out
}

fn kdl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn get_prop(node: &kdl::KdlNode, name: &str) -> Option<String> {
    node.get(name)?.as_string().map(|s| s.to_string())
}

fn get_prop_i64(node: &kdl::KdlNode, name: &str) -> Option<i64> {
    node.get(name)?.as_integer().map(|v| v as i64)
}

fn get_child_value(doc: &kdl::KdlDocument, name: &str) -> Option<String> {
    doc.get(name)?
        .entries()
        .first()?
        .value()
        .as_string()
        .map(|s| s.to_string())
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Kdl(kdl::KdlError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "IO error: {}", e),
            StoreError::Kdl(e) => write!(f, "KDL parse error: {}", e),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_missing_file_returns_empty() {
        let anns = load_annotations(Path::new("/nonexistent/file.kdl")).unwrap();
        assert!(anns.is_empty());
    }

    #[test]
    fn test_kdl_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.kdl");

        let expected = vec![
            Annotation {
                id: "ann-1".into(),
                user: "alice".into(),
                timestamp: 123456789,
                context_hash: "sha256:abc".into(),
                target_text: "Hello".into(),
                body: "World".into(),
            },
            Annotation {
                id: "ann-2".into(),
                user: "bob".into(),
                timestamp: 987654321,
                context_hash: "sha256:def".into(),
                target_text: "Foo".into(),
                body: "Bar".into(),
            },
        ];

        save_annotations(&path, &expected).unwrap();
        let actual = load_annotations(&path).unwrap();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].id, "ann-1");
        assert_eq!(actual[0].user, "alice");
        assert_eq!(actual[0].timestamp, 123456789);
        assert_eq!(actual[0].context_hash, "sha256:abc");
        assert_eq!(actual[0].target_text, "Hello");
        assert_eq!(actual[0].body, "World");

        assert_eq!(actual[1].id, "ann-2");
        assert_eq!(actual[1].body, "Bar");
    }

    #[test]
    fn test_append_upsert_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.kdl");

        let original = vec![
            Annotation {
                id: "ann-1".into(),
                user: "alice".into(),
                timestamp: 100,
                context_hash: "hash1".into(),
                target_text: "Hello".into(),
                body: "World".into(),
            },
        ];
        save_annotations(&path, &original).unwrap();

        // Upsert with same ID
        let updated = Annotation {
            id: "ann-1".into(),
            user: "alice".into(),
            timestamp: 200,
            context_hash: "hash1".into(),
            target_text: "Hello Updated".into(),
            body: "World Updated".into(),
        };
        append_annotation(&path, updated).unwrap();

        let anns = load_annotations(&path).unwrap();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].body, "World Updated");
        assert_eq!(anns[0].target_text, "Hello Updated");
    }

    #[test]
    fn test_append_new() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.kdl");

        let original = vec![
            Annotation {
                id: "ann-1".into(),
                user: "".into(),
                timestamp: 0,
                context_hash: "".into(),
                target_text: "Hello".into(),
                body: "World".into(),
            },
        ];
        save_annotations(&path, &original).unwrap();

        let new_ann = Annotation {
            id: "ann-2".into(),
            user: "".into(),
            timestamp: 0,
            context_hash: "".into(),
            target_text: "New".into(),
            body: "Note".into(),
        };
        append_annotation(&path, new_ann).unwrap();

        let anns = load_annotations(&path).unwrap();
        assert_eq!(anns.len(), 2);
        assert_eq!(anns[1].id, "ann-2");
        assert_eq!(anns[1].body, "Note");
    }

    #[test]
    fn test_append_to_nonexistent_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new.kdl");

        let ann = Annotation {
            id: "first".into(),
            user: "".into(),
            timestamp: 0,
            context_hash: "".into(),
            target_text: "Target".into(),
            body: "Body".into(),
        };
        append_annotation(&path, ann).unwrap();

        let anns = load_annotations(&path).unwrap();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].id, "first");
    }

    #[test]
    fn test_body_with_special_chars() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("special.kdl");

        let ann = vec![Annotation {
            id: "ann-special".into(),
            user: "".into(),
            timestamp: 0,
            context_hash: "".into(),
            target_text: "text with \"quotes\"".into(),
            body: "body with \\backslash".into(),
        }];
        save_annotations(&path, &ann).unwrap();

        let loaded = load_annotations(&path).unwrap();
        assert_eq!(loaded[0].target_text, "text with \"quotes\"");
        assert_eq!(loaded[0].body, "body with \\backslash");
    }
}
