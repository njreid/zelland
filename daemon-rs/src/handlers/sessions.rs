use axum::extract::{Json, State};
use axum::http::StatusCode;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use crate::server::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentSessionEntry {
    pub session_name: String,
    pub connected_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme_excerpt: Option<String>,
}

#[derive(Deserialize)]
pub struct RecordRequest {
    pub session_name: String,
}

fn sessions_file(state: &AppState) -> PathBuf {
    state.config.projects_path.join(".zelland").join("recent_sessions.json")
}

async fn load_sessions(state: &AppState) -> Vec<RecentSessionEntry> {
    let path = sessions_file(state);
    match fs::read_to_string(&path).await {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn save_sessions(state: &AppState, entries: &[RecentSessionEntry]) {
    let path = sessions_file(state);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    if let Ok(s) = serde_json::to_string_pretty(entries) {
        let _ = fs::write(&path, s).await;
    }
}

/// Extract the first non-heading, non-empty paragraph from markdown.
fn readme_excerpt(markdown: &str) -> Option<String> {
    let mut para = String::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("---") || trimmed.starts_with("===") {
            if !para.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.is_empty() {
            if !para.is_empty() {
                break;
            }
            continue;
        }
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(trimmed);
    }
    if para.is_empty() {
        return None;
    }
    let s = strip_inline_md(&para);
    if s.is_empty() {
        return None;
    }
    // Truncate at 200 chars on a char boundary
    let truncated = if s.chars().count() > 200 {
        let end = s.char_indices().nth(200).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}…", &s[..end])
    } else {
        s
    };
    Some(truncated)
}

/// Strip common inline markdown markers (bold, italic, code, links).
fn strip_inline_md(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                // [text](url) → text
                let mut text = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == ']' {
                        closed = true;
                        break;
                    }
                    text.push(ch);
                }
                if closed {
                    out.push_str(&text);
                    if chars.peek() == Some(&'(') {
                        chars.next();
                        for ch in chars.by_ref() {
                            if ch == ')' {
                                break;
                            }
                        }
                    }
                } else {
                    out.push('[');
                    out.push_str(&text);
                }
            }
            '*' | '_' => {
                // skip * and ** / _ and __ markers
                if chars.peek() == Some(&c) {
                    chars.next();
                }
            }
            '`' => {} // skip backtick
            other => out.push(other),
        }
    }
    out.trim().to_string()
}

pub async fn get_recent_sessions(State(state): State<AppState>) -> Json<Vec<RecentSessionEntry>> {
    Json(load_sessions(&state).await)
}

pub async fn record_session(
    State(state): State<AppState>,
    Json(req): Json<RecordRequest>,
) -> StatusCode {
    let mut entries = load_sessions(&state).await;

    // Read README excerpt from the project directory
    let readme_path = state
        .config
        .projects_path
        .join(&req.session_name)
        .join("README.md");
    let excerpt = match fs::read_to_string(&readme_path).await {
        Ok(content) => readme_excerpt(&content),
        Err(_) => None,
    };

    let now = Utc::now().to_rfc3339();

    // Remove stale entry for this session, prepend the new one
    entries.retain(|e| e.session_name != req.session_name);
    entries.insert(
        0,
        RecentSessionEntry {
            session_name: req.session_name,
            connected_at: now,
            readme_excerpt: excerpt,
        },
    );
    entries.truncate(10);

    save_sessions(&state, &entries).await;
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readme_excerpt_basic() {
        let md = "# My Project\n\nThis is the first paragraph.\n\nSecond paragraph.";
        assert_eq!(readme_excerpt(md).as_deref(), Some("This is the first paragraph."));
    }

    #[test]
    fn test_readme_excerpt_no_heading() {
        let md = "Just a plain paragraph.\n\nAnother one.";
        assert_eq!(readme_excerpt(md).as_deref(), Some("Just a plain paragraph."));
    }

    #[test]
    fn test_readme_excerpt_strips_links() {
        let md = "A [link](https://example.com) in text.";
        assert_eq!(readme_excerpt(md).as_deref(), Some("A link in text."));
    }

    #[test]
    fn test_readme_excerpt_strips_bold() {
        let md = "Some **bold** and *italic* text.";
        assert_eq!(readme_excerpt(md).as_deref(), Some("Some bold and italic text."));
    }

    #[test]
    fn test_readme_excerpt_empty() {
        assert_eq!(readme_excerpt("# Only a heading\n\n"), None);
    }

    #[test]
    fn test_readme_excerpt_truncates() {
        let long = "word ".repeat(60); // 300 chars
        let md = format!("# Title\n\n{}", long);
        let result = readme_excerpt(&md).unwrap();
        assert!(result.chars().count() <= 201); // 200 + ellipsis
        assert!(result.ends_with('…'));
    }
}
