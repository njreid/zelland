use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::{info, warn};

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;
use crate::store::{strip_comments_section, ensure_codeblock_anchor};
use std::path::Path;

#[derive(Deserialize)]
pub struct ReadQuery {
    pub path: String,
}

pub async fn read_file(
    State(state): State<AppState>,
    Query(query): Query<ReadQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if query.path.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path is required".into()));
    }

    let abs_path = resolve_filepath(&state, &query.path);
    info!("read_file: resolved '{}' -> '{}'", query.path, abs_path);
    let data = tokio::fs::read(&abs_path).await.map_err(|e| {
        (StatusCode::NOT_FOUND, e.to_string())
    })?;

    Ok(data)
}

#[derive(Deserialize)]
pub struct AnnotateRequest {
    pub path: String,
    pub ann_id: String,
    pub quote: String,
    pub author: String,
    pub body: String,
    /// For code block annotations: "startLine:startChar->endLine:endChar" (0-indexed codepoints).
    pub code_range: Option<String>,
    /// For code block annotations: the handle for the `[comments](#handle)` anchor.
    pub codeblock_handle: Option<String>,
    /// For code block annotations: first ~100 chars of the code block's text content,
    /// used to locate the fenced block in the source.
    pub code_text_prefix: Option<String>,
}

/// Strip common inline Markdown formatting characters (`**`, `__`, `~~`, `*`, `_`, `` ` ``)
/// from `raw`, normalising `\n` to space. Returns (stripped_text, char_map) where
/// `char_map[i]` is the byte offset in `raw` of the i-th character in `stripped_text`.
fn strip_inline_markdown(raw: &str) -> (String, Vec<usize>) {
    let mut stripped = String::new();
    let mut char_map: Vec<usize> = Vec::new();
    let mut pos = 0;

    while pos < raw.len() {
        let remaining = &raw[pos..];
        if remaining.starts_with("**") || remaining.starts_with("__") || remaining.starts_with("~~") {
            pos += 2;
        } else if remaining.starts_with('*') || remaining.starts_with('_') || remaining.starts_with('`') {
            pos += 1;
        } else {
            let ch = remaining.chars().next().unwrap();
            let ch_len = ch.len_utf8();
            stripped.push(if ch == '\n' { ' ' } else { ch });
            char_map.push(pos);
            pos += ch_len;
        }
    }

    (stripped, char_map)
}

/// Map a (stripped_byte_start, stripped_byte_end) range back to raw byte offsets,
/// then widen raw_start/raw_end to absorb immediately-adjacent formatting markers.
fn map_to_raw(
    stripped_start: usize,
    stripped_end: usize,
    stripped: &str,
    char_map: &[usize],
    raw: &str,
) -> (usize, usize) {
    let char_start = stripped[..stripped_start].chars().count();
    let char_end   = stripped[..stripped_end].chars().count();

    let raw_start = char_map.get(char_start).copied().unwrap_or(raw.len());
    let raw_end   = char_map.get(char_end).copied().unwrap_or(raw.len());

    let raw_start = absorb_preceding_markers(raw, raw_start);
    let raw_end   = absorb_following_markers(raw, raw_end);

    (raw_start, raw_end)
}

fn absorb_preceding_markers(raw: &str, pos: usize) -> usize {
    if pos >= 2 {
        let two = &raw[pos - 2..pos];
        if two == "**" || two == "__" || two == "~~" {
            return pos - 2;
        }
    }
    if pos >= 1 {
        let one = raw.as_bytes()[pos - 1];
        if one == b'*' || one == b'_' || one == b'`' {
            return pos - 1;
        }
    }
    pos
}

fn absorb_following_markers(raw: &str, pos: usize) -> usize {
    if pos + 2 <= raw.len() {
        let two = &raw[pos..pos + 2];
        if two == "**" || two == "__" || two == "~~" {
            return pos + 2;
        }
    }
    if pos + 1 <= raw.len() {
        let one = raw.as_bytes()[pos];
        if one == b'*' || one == b'_' || one == b'`' {
            return pos + 1;
        }
    }
    pos
}

/// Find `rendered_quote` in `raw` markdown, ignoring inline formatting markers
/// and treating any whitespace as equivalent to any whitespace.
/// Returns (raw_start, raw_end) byte positions.
fn find_in_markdown(raw: &str, rendered_quote: &str) -> Option<(usize, usize)> {
    let (stripped, char_map) = strip_inline_markdown(raw);

    // Fast path: direct substring match (handles bold/italic stripping)
    if let Some(start) = stripped.find(rendered_quote) {
        let end = start + rendered_quote.len();
        return Some(map_to_raw(start, end, &stripped, &char_map, raw));
    }

    // Fallback: whitespace-normalised match (handles \n vs space, extra spaces)
    let parts: Vec<&str> = rendered_quote.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    let first = parts[0];
    let mut search_start = 0;
    while search_start < stripped.len() {
        let Some(rel) = stripped[search_start..].find(first) else { break };
        let abs = search_start + rel;
        let mut cursor = abs + first.len();
        let mut matched = true;
        for part in &parts[1..] {
            let ws = stripped[cursor..].len() - stripped[cursor..].trim_start().len();
            if ws == 0 { matched = false; break; }
            cursor += ws;
            if !stripped[cursor..].starts_with(part) { matched = false; break; }
            cursor += part.len();
        }
        if matched {
            return Some(map_to_raw(abs, cursor, &stripped, &char_map, raw));
        }
        search_start = abs + 1;
    }

    None
}

pub async fn annotate_file(
    State(state): State<AppState>,
    Json(req): Json<AnnotateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if req.path.is_empty() || req.ann_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path and ann_id are required".into()));
    }

    let abs_path_str = resolve_filepath(&state, &req.path);
    info!("annotate_file: resolved '{}' -> '{}'", req.path, abs_path_str);
    let abs_path = Path::new(&abs_path_str);

    if !abs_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("File not found: {}", abs_path_str)));
    }

    let content = tokio::fs::read_to_string(&abs_path).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Determine the actual codeblock_handle used (server may reuse an existing anchor).
    let actual_codeblock_handle: Option<String>;

    if let (Some(code_range), Some(handle), Some(prefix)) =
        (&req.code_range, &req.codeblock_handle, &req.code_text_prefix)
    {
        // Code block annotation: insert/confirm [comments](#handle) anchor after the fence.
        match ensure_codeblock_anchor(&content, prefix, handle) {
            Some((new_content, used_handle)) => {
                if new_content != content {
                    tokio::fs::write(&abs_path, &new_content).await.map_err(|e| {
                        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                    })?;
                    info!(
                        "annotate_file: inserted codeblock anchor '{}' for ann '{}'",
                        used_handle, req.ann_id
                    );
                } else {
                    info!(
                        "annotate_file: reused existing codeblock anchor '{}' for ann '{}'",
                        used_handle, req.ann_id
                    );
                }
                actual_codeblock_handle = Some(used_handle);
            }
            None => {
                warn!(
                    "annotate_file: could not find code block with prefix {:?} in '{}' — falling back to Comments-only",
                    prefix, abs_path_str
                );
                actual_codeblock_handle = Some(handle.clone());
            }
        }
        drop(code_range); // suppress unused warning
    } else {
        // Regular (prose) annotation: insert (quote)[#ann_id] inline in the body.
        actual_codeblock_handle = None;

        let body = strip_comments_section(&content);
        let body_len = body.len();

        if let Some((raw_start, raw_end)) = find_in_markdown(body, &req.quote) {
            let raw_start = raw_start.min(body_len);
            let raw_end = raw_end.min(body_len);

            let replacement = format!("({})[#{}]", req.quote, req.ann_id);
            let new_content = format!(
                "{}{}{}",
                &content[..raw_start],
                replacement,
                &content[raw_end..]
            );
            tokio::fs::write(&abs_path, &new_content).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;
            info!(
                "annotate_file: inserted inline marker for '{}' at [{}, {})",
                req.ann_id, raw_start, raw_end
            );
        } else {
            warn!(
                "annotate_file: quote not found in body of '{}', falling back to Comments-only. quote={:?}",
                abs_path_str, req.quote
            );
        }
    }

    // Store annotation in Loro (also appends/updates the ## Comments section).
    state.loro_manager.add_annotation(
        abs_path,
        req.ann_id,
        req.quote,
        req.author,
        req.body,
        req.code_range,
        actual_codeblock_handle,
    ).await;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_plain_text() {
        let raw = "Some plain text here.";
        let (start, end) = find_in_markdown(raw, "plain text").unwrap();
        assert_eq!(&raw[start..end], "plain text");
    }

    #[test]
    fn test_find_strips_bold() {
        let raw = "- **Hardware**: 64x32 RGB LED matrix display";
        let quote = "Hardware: 64x32 RGB LED matrix display";
        let (start, end) = find_in_markdown(raw, quote).unwrap();
        assert!(raw[start..end].contains("Hardware"));
        assert!(start < end);
    }

    #[test]
    fn test_find_whitespace_normalised() {
        let raw = "foo\nbar baz";
        let quote = "foo bar";
        let (start, end) = find_in_markdown(raw, quote).unwrap();
        assert!(start < end);
        assert!(raw[start..end].starts_with("foo"));
    }

    #[test]
    fn test_find_not_in_comments_section() {
        let raw = "# Title\n\nBody content.\n\n---\n\n## Comments\n\n### x\n\n> `Body content.`\n";
        let body = strip_comments_section(raw);
        assert!(find_in_markdown(body, "Body content.").is_some());
    }

    #[test]
    fn test_find_returns_none_for_missing() {
        let raw = "Some text here.";
        assert!(find_in_markdown(raw, "not present").is_none());
    }
}
