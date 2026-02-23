use std::path::{Path, PathBuf};
use std::fs;
use crate::handlers::annotations::Comment;

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "IO error: {}", e),
            StoreError::Parse(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for StoreError {}

/// Parse the `# Comments` section from a Markdown file.
pub fn parse_markdown_comments(path: &Path) -> Result<Vec<(String, Vec<Comment>)>, StoreError> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Io(e)),
    };

    let mut annotations = Vec::new();
    let mut in_comments_section = false;
    let mut current_ann_id: Option<String> = None;
    let mut current_comments = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "# Comments" {
            in_comments_section = true;
            continue;
        }

        if !in_comments_section {
            continue;
        }

        if trimmed.starts_with("## ") {
            if let Some(id) = current_ann_id.take() {
                annotations.push((id, current_comments));
                current_comments = Vec::new();
            }
            current_ann_id = Some(trimmed[3..].to_string());
        } else if trimmed.starts_with("- ") {
            if let Some(comment) = parse_comment_line(trimmed) {
                current_comments.push(comment);
            }
        }
    }

    if let Some(id) = current_ann_id {
        annotations.push((id, current_comments));
    }

    Ok(annotations)
}

fn parse_comment_line(line: &str) -> Option<Comment> {
    // Format: "- TIMESTAMP author: body"
    let content = &line[2..];
    let first_space = content.find(' ')?;
    let timestamp = content[..first_space].to_string();
    let remaining = &content[first_space + 1..];
    let colon = remaining.find(':')?;
    let author = remaining[..colon].to_string();
    let body = remaining[colon + 1..].trim().to_string();

    Some(Comment {
        author,
        timestamp,
        body,
    })
}

/// Reify the annotations back into the Markdown file's `# Comments` section.
pub fn reify_markdown_comments(path: &Path, annotations: &[(String, Vec<Comment>)]) -> Result<(), StoreError> {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    
    for line in content.lines() {
        if line.trim() == "# Comments" {
            break;
        }
        lines.push(line.to_string());
    }

    // Ensure there's a blank line before # Comments if not already there
    if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
        lines.push("".to_string());
    }

    lines.push("# Comments".to_string());
    lines.push("".to_string());

    for (id, thread) in annotations {
        lines.push(format!("## {}", id));
        lines.push("".to_string());
        for comment in thread {
            lines.push(format!("- {} {}: {}", comment.timestamp, comment.author, comment.body));
        }
        lines.push("".to_string());
    }

    let new_content = lines.join("\n");
    fs::write(path, new_content).map_err(StoreError::Io)
}

/// Path to the hidden loro history cache: `.<filename>.zelland`
pub fn loro_cache_path(source_path: &Path) -> PathBuf {
    let filename = source_path.file_name().unwrap_or_default().to_string_lossy();
    source_path.with_file_name(format!(".{}.zelland", filename))
}

#[cfg(test)]
mod store_tests;
