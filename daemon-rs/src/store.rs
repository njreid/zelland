use std::path::Path;

// --- New annotation types (for .ann.kdl format, used by YJS sync) ---

/// A comment in an annotation thread.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub created: String,
    pub body: String,
}

/// Text selector for anchoring an annotation to document content.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Selector {
    pub quote: String,
    pub prefix: String,
    pub suffix: String,
}

/// A rich annotation with selector and threaded comments.
/// Stored in `.ann.kdl` sidecar files.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ann {
    pub id: String,
    pub selector: Selector,
    pub thread: Vec<Comment>,
}

/// Load annotations from a `.ann.kdl` file. Returns empty vec if file doesn't exist.
pub fn load_anns(path: &Path) -> Result<Vec<Ann>, StoreError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Io(e)),
    };
    parse_anns(&content)
}

/// Save annotations to a `.ann.kdl` file, overwriting any existing content.
pub fn save_anns(path: &Path, anns: &[Ann]) -> Result<(), StoreError> {
    let content = serialize_anns(anns);
    std::fs::write(path, content).map_err(StoreError::Io)
}

fn parse_anns(input: &str) -> Result<Vec<Ann>, StoreError> {
    let doc: kdl::KdlDocument = input.parse().map_err(StoreError::Kdl)?;
    let mut anns = Vec::new();

    for node in doc.nodes() {
        if node.name().value() != "ann" {
            continue;
        }

        let id = node
            .entries()
            .first()
            .and_then(|e| e.value().as_string())
            .unwrap_or("")
            .to_string();

        let children = match node.children() {
            Some(c) => c,
            None => continue,
        };

        // Parse selector
        let selector = children
            .get("selector")
            .and_then(|s| s.children())
            .map(|sc| Selector {
                quote: get_child_value(sc, "quote").unwrap_or_default(),
                prefix: get_child_value(sc, "prefix").unwrap_or_default(),
                suffix: get_child_value(sc, "suffix").unwrap_or_default(),
            })
            .unwrap_or(Selector {
                quote: String::new(),
                prefix: String::new(),
                suffix: String::new(),
            });

        // Parse thread
        let mut thread = Vec::new();
        if let Some(thread_node) = children.get("thread") {
            if let Some(thread_children) = thread_node.children() {
                for comment_node in thread_children.nodes() {
                    if comment_node.name().value() != "comment" {
                        continue;
                    }
                    let comment_id = get_prop(comment_node, "id").unwrap_or_default();
                    let author = get_prop(comment_node, "author").unwrap_or_default();
                    let created = get_prop(comment_node, "created").unwrap_or_default();
                    let body = comment_node
                        .children()
                        .and_then(|c| get_child_value(c, "body"))
                        .unwrap_or_default();

                    thread.push(Comment {
                        id: comment_id,
                        author,
                        created,
                        body,
                    });
                }
            }
        }

        anns.push(Ann {
            id,
            selector,
            thread,
        });
    }

    Ok(anns)
}

fn serialize_anns(anns: &[Ann]) -> String {
    let mut out = String::new();
    for ann in anns {
        out.push_str(&format!("ann {} {{\n", kdl_quote(&ann.id)));
        out.push_str("    selector {\n");
        out.push_str(&format!("        quote {}\n", kdl_quote(&ann.selector.quote)));
        out.push_str(&format!(
            "        prefix {}\n",
            kdl_quote(&ann.selector.prefix)
        ));
        out.push_str(&format!(
            "        suffix {}\n",
            kdl_quote(&ann.selector.suffix)
        ));
        out.push_str("    }\n");
        out.push_str("    thread {\n");
        for comment in &ann.thread {
            out.push_str(&format!(
                "        comment id={} author={} created={} {{\n",
                kdl_quote(&comment.id),
                kdl_quote(&comment.author),
                kdl_quote(&comment.created),
            ));
            out.push_str(&format!("            body {}\n", kdl_quote(&comment.body)));
            out.push_str("        }\n");
        }
        out.push_str("    }\n");
        out.push_str("}\n");
    }
    out
}

/// Derive the `.ann.kdl` sidecar path from a source file path.
/// `README.md` → `README.ann.kdl`, `docs/notes.txt` → `docs/notes.ann.kdl`
pub fn ann_kdl_path(source_path: &Path) -> std::path::PathBuf {
    let stem = source_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    source_path.with_file_name(format!("{}.ann.kdl", stem))
}

// --- Legacy annotation types (for protobuf WebSocket compat) ---

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

    // --- .ann.kdl format tests ---

    #[test]
    fn test_load_anns_missing_file() {
        let anns = load_anns(Path::new("/nonexistent/file.ann.kdl")).unwrap();
        assert!(anns.is_empty());
    }

    #[test]
    fn test_ann_kdl_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ann.kdl");

        let expected = vec![
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
                        body: "Yes - measured at **45uA** in deep sleep.".into(),
                    },
                ],
            },
            Ann {
                id: "m3x9p".into(),
                selector: Selector {
                    quote: "userspace packet loop".into(),
                    prefix: "Implement ".into(),
                    suffix: " in `src-tauri".into(),
                },
                thread: vec![Comment {
                    id: "c003".into(),
                    author: "njr".into(),
                    created: "2026-02-11T09:00:00Z".into(),
                    body: "This is done - see `network.rs`.".into(),
                }],
            },
        ];

        save_anns(&path, &expected).unwrap();
        let actual = load_anns(&path).unwrap();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].id, "k8f2a");
        assert_eq!(actual[0].selector.quote, "ESP32-S3");
        assert_eq!(actual[0].selector.prefix, "architecture of the ");
        assert_eq!(actual[0].selector.suffix, " microcontroller");
        assert_eq!(actual[0].thread.len(), 2);
        assert_eq!(actual[0].thread[0].id, "c001");
        assert_eq!(actual[0].thread[0].author, "njr");
        assert_eq!(actual[0].thread[0].body, "Should we verify the power draw?");
        assert_eq!(actual[0].thread[1].id, "c002");
        assert_eq!(actual[0].thread[1].body, "Yes - measured at **45uA** in deep sleep.");

        assert_eq!(actual[1].id, "m3x9p");
        assert_eq!(actual[1].thread.len(), 1);
    }

    #[test]
    fn test_ann_kdl_special_chars() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("special.ann.kdl");

        let expected = vec![Ann {
            id: "sp1".into(),
            selector: Selector {
                quote: "text with \"quotes\"".into(),
                prefix: "before\\".into(),
                suffix: "after".into(),
            },
            thread: vec![Comment {
                id: "c1".into(),
                author: "user".into(),
                created: "2026-01-01T00:00:00Z".into(),
                body: "body with \\backslash and \"quotes\"".into(),
            }],
        }];

        save_anns(&path, &expected).unwrap();
        let actual = load_anns(&path).unwrap();

        assert_eq!(actual[0].selector.quote, "text with \"quotes\"");
        assert_eq!(actual[0].selector.prefix, "before\\");
        assert_eq!(actual[0].thread[0].body, "body with \\backslash and \"quotes\"");
    }

    #[test]
    fn test_ann_kdl_empty_thread() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty_thread.ann.kdl");

        let expected = vec![Ann {
            id: "e1".into(),
            selector: Selector {
                quote: "some text".into(),
                prefix: "".into(),
                suffix: "".into(),
            },
            thread: Vec::new(),
        }];

        save_anns(&path, &expected).unwrap();
        let actual = load_anns(&path).unwrap();

        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].id, "e1");
        assert!(actual[0].thread.is_empty());
    }

    #[test]
    fn test_ann_kdl_path_derivation() {
        assert_eq!(
            ann_kdl_path(Path::new("README.md")),
            Path::new("README.ann.kdl")
        );
        assert_eq!(
            ann_kdl_path(Path::new("docs/notes.txt")),
            Path::new("docs/notes.ann.kdl")
        );
        assert_eq!(
            ann_kdl_path(Path::new("/absolute/path/file.md")),
            Path::new("/absolute/path/file.ann.kdl")
        );
    }

    #[test]
    fn test_ann_serde_json_roundtrip() {
        let ann = Ann {
            id: "test".into(),
            selector: Selector {
                quote: "hello".into(),
                prefix: "".into(),
                suffix: "".into(),
            },
            thread: vec![Comment {
                id: "c1".into(),
                author: "alice".into(),
                created: "2026-01-01T00:00:00Z".into(),
                body: "note".into(),
            }],
        };
        let json = serde_json::to_string(&ann).unwrap();
        let parsed: Ann = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ann);
    }
}
