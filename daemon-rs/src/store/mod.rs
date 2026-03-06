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

/// A single annotation: id, quote, thread, and optional code block metadata.
#[derive(Debug, Clone)]
pub struct Annotation {
    pub id: String,
    pub quote: String,
    pub thread: Vec<Comment>,
    /// For code block annotations: "startLine:startChar->endLine:endChar" (0-indexed codepoints).
    pub code_range: Option<String>,
    /// For code block annotations: the handle used in `[comments](#handle)` anchor.
    pub codeblock_handle: Option<String>,
}

impl Annotation {
    pub fn regular(id: String, quote: String, thread: Vec<Comment>) -> Self {
        Self { id, quote, thread, code_range: None, codeblock_handle: None }
    }

    pub fn code(
        id: String,
        quote: String,
        thread: Vec<Comment>,
        code_range: String,
        codeblock_handle: String,
    ) -> Self {
        Self { id, quote, thread, code_range: Some(code_range), codeblock_handle: Some(codeblock_handle) }
    }

    pub fn is_code(&self) -> bool {
        self.codeblock_handle.is_some()
    }
}

/// Deterministic ID for a code block annotation based on its (handle, range) pair.
/// Allows deduplication when loading from disk (no Loro cache).
pub fn code_ann_id(codeblock_handle: &str, code_range: &str) -> String {
    let h: String = codeblock_handle.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let r: String = code_range.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    format!("c_{}_{}", h, r)
}

/// Parse the `# Comments` / `## Comments` section from a Markdown file.
///
/// Supports both regular annotations (`### ann_id` with direct quote/comments)
/// and code block annotations (`### codeblock_handle` with `#### range` subsections).
pub fn parse_markdown_comments(path: &Path) -> Result<Vec<Annotation>, StoreError> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Io(e)),
    };

    let mut annotations = Vec::new();
    let mut in_comments_section = false;

    // Current ### level state
    let mut current_section_id: Option<String> = None;
    let mut current_section_is_code = false;

    // Current item state (a ### regular section OR a #### code sub-section)
    let mut current_item_range: Option<String> = None;
    let mut current_quote = String::new();
    let mut current_comments: Vec<Comment> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "# Comments" || trimmed == "## Comments" {
            in_comments_section = true;
            continue;
        }

        if !in_comments_section {
            continue;
        }

        if trimmed.starts_with("### ") {
            // Flush previous item before starting a new ### section
            flush_annotation(
                &mut annotations,
                current_section_id.as_deref(),
                current_section_is_code,
                current_item_range.take(),
                std::mem::take(&mut current_quote),
                std::mem::take(&mut current_comments),
            );
            current_section_id = Some(trimmed[4..].to_string());
            current_section_is_code = false;
        } else if trimmed.starts_with("## ") {
            // Old format backwards compat: ## ann_id (not "## Comments")
            flush_annotation(
                &mut annotations,
                current_section_id.as_deref(),
                current_section_is_code,
                current_item_range.take(),
                std::mem::take(&mut current_quote),
                std::mem::take(&mut current_comments),
            );
            current_section_id = Some(trimmed[3..].to_string());
            current_section_is_code = false;
        } else if trimmed.starts_with("#### ") {
            // Code block subsection — flush the previous code item (same ### group)
            if current_section_is_code && current_item_range.is_some() {
                flush_annotation(
                    &mut annotations,
                    current_section_id.as_deref(),
                    true,
                    current_item_range.take(),
                    std::mem::take(&mut current_quote),
                    std::mem::take(&mut current_comments),
                );
            }
            current_section_is_code = true;
            current_item_range = Some(trimmed[5..].to_string());
        } else if trimmed.starts_with("> ") && current_section_id.is_some() {
            let raw_quote = trimmed[2..].trim();
            current_quote = raw_quote.trim_matches('`').to_string();
        } else if trimmed.starts_with("- ") {
            if let Some(comment) = parse_comment_line(trimmed) {
                current_comments.push(comment);
            }
        }
    }

    // Flush the final item
    flush_annotation(
        &mut annotations,
        current_section_id.as_deref(),
        current_section_is_code,
        current_item_range.take(),
        current_quote,
        current_comments,
    );

    Ok(annotations)
}

fn flush_annotation(
    annotations: &mut Vec<Annotation>,
    section_id: Option<&str>,
    section_is_code: bool,
    item_range: Option<String>,
    quote: String,
    comments: Vec<Comment>,
) {
    let Some(sid) = section_id else { return };

    if section_is_code {
        if let Some(range) = item_range {
            let ann_id = code_ann_id(sid, &range);
            annotations.push(Annotation::code(ann_id, quote, comments, range, sid.to_string()));
        }
    } else {
        // Regular annotation — always push (preserves backwards compat with empty entries)
        annotations.push(Annotation::regular(sid.to_string(), quote, comments));
    }
}

fn parse_comment_line(line: &str) -> Option<Comment> {
    // Format: "- TIMESTAMP author: body"
    if !line.starts_with("- ") { return None; }
    let content = &line[2..];
    let first_space = content.find(' ')?;
    let timestamp = content[..first_space].to_string();
    let remaining = &content[first_space + 1..];
    let colon = remaining.find(':')?;
    let author = remaining[..colon].to_string();
    let body = remaining[colon + 1..].trim().to_string();
    Some(Comment { author, timestamp, body })
}

/// Reify annotations back into the Markdown file's `## Comments` section.
///
/// Regular annotations are written as `### ann_id`.
/// Code block annotations are grouped by `codeblock_handle` under `### handle`
/// with `#### startLine:startChar->endLine:endChar` subsections.
pub fn reify_markdown_comments(path: &Path, annotations: &[Annotation]) -> Result<(), StoreError> {
    let content = fs::read_to_string(path).unwrap_or_default();
    let body = strip_comments_section(&content).trim_end().to_string();
    let mut result = body;

    if !annotations.is_empty() {
        result.push_str("\n\n---\n\n## Comments\n");

        // Separate regular from code annotations
        let regular: Vec<&Annotation> = annotations.iter().filter(|a| !a.is_code()).collect();

        // Group code annotations by codeblock_handle (preserving first-seen order)
        let mut code_groups: Vec<(String, Vec<&Annotation>)> = Vec::new();
        for ann in annotations.iter().filter(|a| a.is_code()) {
            let handle = ann.codeblock_handle.as_deref().unwrap_or("unknown");
            if let Some(group) = code_groups.iter_mut().find(|(h, _)| h == handle) {
                group.1.push(ann);
            } else {
                code_groups.push((handle.to_string(), vec![ann]));
            }
        }

        // Write regular annotations
        for ann in &regular {
            result.push_str(&format!("\n### {}\n\n", ann.id));
            if !ann.quote.is_empty() {
                result.push_str(&format!("> `{}`\n\n", ann.quote));
            }
            for comment in &ann.thread {
                result.push_str(&format!(
                    "- {} {}: {}\n",
                    comment.timestamp, comment.author, comment.body
                ));
            }
        }

        // Write code block groups
        for (handle, anns) in &code_groups {
            result.push_str(&format!("\n### {}\n", handle));
            let mut sorted = anns.to_vec();
            sorted.sort_by(|a, b| a.code_range.cmp(&b.code_range));
            for ann in sorted {
                let range = ann.code_range.as_deref().unwrap_or("0:0->0:0");
                result.push_str(&format!("\n#### {}\n\n", range));
                if !ann.quote.is_empty() {
                    result.push_str(&format!("> `{}`\n\n", ann.quote));
                }
                for comment in &ann.thread {
                    result.push_str(&format!(
                        "- {} {}: {}\n",
                        comment.timestamp, comment.author, comment.body
                    ));
                }
            }
        }
    }

    fs::write(path, result).map_err(StoreError::Io)
}

/// Ensure a `[comments](#handle)` anchor exists immediately after the fenced code block
/// whose content starts with `code_text_prefix`.
///
/// - Returns `Some((content, handle))` if the block was found.
///   If the block already has an anchor, the existing handle is returned unchanged.
///   If not, `proposed_handle` is inserted and returned.
/// - Returns `None` if the code block cannot be located.
pub fn ensure_codeblock_anchor(
    content: &str,
    code_text_prefix: &str,
    proposed_handle: &str,
) -> Option<(String, String)> {
    if code_text_prefix.is_empty() {
        return None;
    }

    let body = strip_comments_section(content);
    let closing_pos = find_fenced_block_end(body, code_text_prefix)?;
    let after_fence = &body[closing_pos..];

    // Reuse existing anchor if present
    if let Some(existing) = find_codeblock_anchor_at(after_fence) {
        return Some((content.to_string(), existing));
    }

    // Insert anchor immediately after the closing fence line
    let anchor = format!("[comments](#{})\n", proposed_handle);
    let new_content = format!("{}{}{}", &content[..closing_pos], anchor, &content[closing_pos..]);
    Some((new_content, proposed_handle.to_string()))
}

/// Find `[comments](#handle)` within the first few lines of `text`.
fn find_codeblock_anchor_at(text: &str) -> Option<String> {
    for line in text.lines().take(3) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("[comments](#") {
            if let Some(end) = rest.find(')') {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// Return the byte position in `body` immediately after the closing fence line
/// of the fenced code block whose content starts with `code_text_prefix`.
fn find_fenced_block_end(body: &str, code_text_prefix: &str) -> Option<usize> {
    let mut byte_pos = 0;

    while byte_pos < body.len() {
        let line_end = body[byte_pos..]
            .find('\n')
            .map(|p| byte_pos + p + 1)
            .unwrap_or(body.len());

        let line = body[byte_pos..line_end].trim_end_matches(['\n', '\r']);
        let line_trimmed = line.trim_start();

        let (is_fence, fence_char, fence_len) = detect_fence(line_trimmed);
        if is_fence {
            let code_start = line_end;
            let mut inner_pos = code_start;
            let mut found_close = false;

            while inner_pos < body.len() {
                let inner_line_end = body[inner_pos..]
                    .find('\n')
                    .map(|p| inner_pos + p + 1)
                    .unwrap_or(body.len());

                let inner_line = body[inner_pos..inner_line_end].trim_end_matches(['\n', '\r']);
                let inner_trimmed = inner_line.trim_start();

                // Count the leading fence characters
                let close_run = inner_trimmed.chars().take_while(|&c| c == fence_char).count();
                if close_run >= fence_len && inner_trimmed[close_run..].trim().is_empty() {
                    // Closing fence — check content match
                    let code_raw = &body[code_start..inner_pos];
                    // Normalize \r\n → \n to match browser textContent
                    let code_cmp = if code_raw.contains('\r') {
                        code_raw.replace("\r\n", "\n")
                    } else {
                        code_raw.to_string()
                    };
                    if code_cmp.starts_with(code_text_prefix) {
                        return Some(inner_line_end);
                    }
                    // Wrong block — continue outer loop from after this block
                    byte_pos = inner_line_end;
                    found_close = true;
                    break;
                }

                inner_pos = inner_line_end;
            }

            if !found_close {
                // Unclosed fence — skip to end
                byte_pos = body.len();
            }
        } else {
            byte_pos = line_end;
        }
    }

    None
}

fn detect_fence(line: &str) -> (bool, char, usize) {
    if line.starts_with("```") {
        let len = line.chars().take_while(|&c| c == '`').count();
        return (true, '`', len);
    }
    if line.starts_with("~~~") {
        let len = line.chars().take_while(|&c| c == '~').count();
        return (true, '~', len);
    }
    (false, ' ', 0)
}

/// Find the earliest position in `content` where the comments section begins,
/// including any preceding "---" separator.
pub fn strip_comments_section(content: &str) -> &str {
    let patterns = [
        "\n---\n\n## Comments",
        "\n---\n## Comments",
        "\n---\n\n# Comments",
        "\n---\n# Comments",
        "\n# Comments",
        "\n## Comments",
    ];
    let mut earliest = content.len();
    for pat in &patterns {
        if let Some(pos) = content.find(pat) {
            if pos < earliest {
                earliest = pos;
            }
        }
    }
    &content[..earliest]
}

/// Path to the hidden loro history cache: `.<filename>.zelland`
pub fn loro_cache_path(source_path: &Path) -> PathBuf {
    let filename = source_path.file_name().unwrap_or_default().to_string_lossy();
    source_path.with_file_name(format!(".{}.zelland", filename))
}

#[cfg(test)]
mod store_tests;
