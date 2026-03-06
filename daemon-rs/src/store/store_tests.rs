use std::fs;
use crate::store::{
    parse_markdown_comments, reify_markdown_comments, ensure_codeblock_anchor, Annotation,
};
use crate::handlers::annotations::Comment;
use tempfile::tempdir;

fn make_comment(author: &str, ts: &str, body: &str) -> Comment {
    Comment {
        author: author.to_string(),
        timestamp: ts.to_string(),
        body: body.to_string(),
    }
}

#[test]
fn test_markdown_comments_roundtrip() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let initial_content = "# Title\n\nSome content here.\n";
    fs::write(&file_path, initial_content).unwrap();

    let anns = vec![Annotation::regular(
        "ann1".to_string(),
        "Some content here.".to_string(),
        vec![
            make_comment("njr", "2026-02-11T09:00:00Z", "First comment"),
            make_comment("bot", "2026-02-12T09:00:00Z", "Second comment"),
        ],
    )];

    reify_markdown_comments(&file_path, &anns).unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("## Comments"), "missing ## Comments header");
    assert!(content.contains("### ann1"), "missing ### ann1");
    assert!(content.contains("> `Some content here.`"), "missing backtick-wrapped blockquote");
    assert!(content.contains("- 2026-02-11T09:00:00Z njr: First comment"));
    assert!(content.contains("---"), "missing --- separator");

    let parsed = parse_markdown_comments(&file_path).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "ann1");
    assert_eq!(parsed[0].quote, "Some content here.");
    assert_eq!(parsed[0].thread.len(), 2);
    assert_eq!(parsed[0].thread[0].author, "njr");
    assert_eq!(parsed[0].thread[1].body, "Second comment");
    assert!(parsed[0].code_range.is_none());
    assert!(parsed[0].codeblock_handle.is_none());
}

#[test]
fn test_reify_overwrites_existing_comments() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let content = "# Title\n\nContent\n\n# Comments\n\n## old\n- 2026-01-01T00:00:00Z anon: old\n";
    fs::write(&file_path, content).unwrap();

    let new_anns = vec![Annotation::regular(
        "new".to_string(),
        "some quote".to_string(),
        vec![make_comment("user", "2026-02-23T00:00:00Z", "new comment")],
    )];

    reify_markdown_comments(&file_path, &new_anns).unwrap();

    let final_content = fs::read_to_string(&file_path).unwrap();
    assert!(!final_content.contains("## old"), "old annotation should be gone");
    assert!(final_content.contains("### new"), "new annotation should be present");
    assert!(final_content.contains("> `some quote`"), "backtick-wrapped quote should be present");
}

#[test]
fn test_reify_strips_old_format_separator() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let content = "# Title\n\nBody text.\n\n---\n\n## Comments\n\n### x1\n\n> `old quote`\n\n- 2026-01-01T00:00:00Z user: msg\n";
    fs::write(&file_path, content).unwrap();

    let updated_anns = vec![Annotation::regular(
        "x1".to_string(),
        "new quote".to_string(),
        vec![make_comment("user", "2026-01-02T00:00:00Z", "updated")],
    )];

    reify_markdown_comments(&file_path, &updated_anns).unwrap();

    let result = fs::read_to_string(&file_path).unwrap();
    assert!(result.contains("Body text."));
    assert_eq!(result.matches("---").count(), 1, "only one --- separator expected");
    assert!(!result.contains("> `old quote`"));
    assert!(result.contains("> `new quote`"));
}

#[test]
fn test_parse_old_format_backwards_compat() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let content = "# Title\n\n# Comments\n\n## abc123\n\n- 2026-01-01T00:00:00Z alice: hello\n";
    fs::write(&file_path, content).unwrap();

    let parsed = parse_markdown_comments(&file_path).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "abc123");
    assert_eq!(parsed[0].quote, "", "old format has no quote");
    assert_eq!(parsed[0].thread.len(), 1);
    assert_eq!(parsed[0].thread[0].author, "alice");
}

#[test]
fn test_code_block_annotation_roundtrip() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let content = "# Title\n\n```python\ndef foo():\n    pass\n```\n[comments](#cb1)\n";
    fs::write(&file_path, content).unwrap();

    let anns = vec![Annotation::code(
        "c_cb1_0_0__0_7".to_string(),
        "def foo".to_string(),
        vec![make_comment("user", "2026-02-23T00:00:00Z", "looks good")],
        "0:0->0:7".to_string(),
        "cb1".to_string(),
    )];

    reify_markdown_comments(&file_path, &anns).unwrap();

    let result = fs::read_to_string(&file_path).unwrap();
    assert!(result.contains("### cb1"), "missing ### cb1");
    assert!(result.contains("#### 0:0->0:7"), "missing #### range");
    assert!(result.contains("> `def foo`"), "missing code quote");
    assert!(result.contains("- 2026-02-23T00:00:00Z user: looks good"));

    // Parse back
    let parsed = parse_markdown_comments(&file_path).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].codeblock_handle.as_deref(), Some("cb1"));
    assert_eq!(parsed[0].code_range.as_deref(), Some("0:0->0:7"));
    assert_eq!(parsed[0].quote, "def foo");
    assert_eq!(parsed[0].thread[0].body, "looks good");
}

#[test]
fn test_code_block_multiple_annotations_same_block() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    let content = "# Body\n";
    fs::write(&file_path, content).unwrap();

    let anns = vec![
        Annotation::code(
            "c_cb1_0_0__0_5".to_string(),
            "def foo".to_string(),
            vec![make_comment("alice", "2026-01-01T00:00:00Z", "first")],
            "0:0->0:5".to_string(),
            "cb1".to_string(),
        ),
        Annotation::code(
            "c_cb1_1_4__1_9".to_string(),
            "    pass".to_string(),
            vec![make_comment("bob", "2026-01-02T00:00:00Z", "second")],
            "1:4->1:9".to_string(),
            "cb1".to_string(),
        ),
    ];

    reify_markdown_comments(&file_path, &anns).unwrap();

    let result = fs::read_to_string(&file_path).unwrap();
    // Both should be under the same ### cb1
    assert_eq!(result.matches("### cb1").count(), 1, "should be one ### cb1 header");
    assert!(result.contains("#### 0:0->0:5"));
    assert!(result.contains("#### 1:4->1:9"));

    let parsed = parse_markdown_comments(&file_path).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(parsed.iter().all(|a| a.codeblock_handle.as_deref() == Some("cb1")));
}

#[test]
fn test_ensure_codeblock_anchor_inserts() {
    let content = "# Title\n\n```python\ndef foo():\n    pass\n```\nSome text after.\n";
    let prefix = "def foo():";

    let (new_content, handle) = ensure_codeblock_anchor(content, prefix, "cb99").unwrap();
    assert_eq!(handle, "cb99");
    assert!(new_content.contains("[comments](#cb99)"));

    // Anchor should appear before "Some text after."
    let anchor_pos = new_content.find("[comments](#cb99)").unwrap();
    let text_pos = new_content.find("Some text after.").unwrap();
    assert!(anchor_pos < text_pos);
}

#[test]
fn test_ensure_codeblock_anchor_reuses_existing() {
    let content = "# Title\n\n```python\ndef foo():\n    pass\n```\n[comments](#existing)\nSome text.\n";
    let prefix = "def foo():";

    let (returned_content, handle) = ensure_codeblock_anchor(content, prefix, "new_handle").unwrap();
    // Should reuse existing handle
    assert_eq!(handle, "existing");
    // Content should be unchanged
    assert_eq!(returned_content, content);
    assert!(!returned_content.contains("[comments](#new_handle)"));
}

#[test]
fn test_ensure_codeblock_anchor_not_found() {
    let content = "# Title\n\nNo code here.\n";
    assert!(ensure_codeblock_anchor(content, "def foo():", "cb1").is_none());
}

#[test]
fn test_ensure_codeblock_anchor_empty_prefix() {
    let content = "# Title\n\n```\nsome code\n```\n";
    assert!(ensure_codeblock_anchor(content, "", "cb1").is_none());
}

#[test]
fn test_find_fenced_block_skips_wrong_blocks() {
    // Two code blocks; prefix matches the second one
    let content = "```python\nprint('hello')\n```\n\n```rust\nfn main() {}\n```\n";
    let prefix = "fn main()";

    let (new_content, handle) = ensure_codeblock_anchor(content, prefix, "cbrust").unwrap();
    assert_eq!(handle, "cbrust");
    // Anchor should appear after the rust block, not after the python block
    let python_end = new_content.find("print('hello')").unwrap() + "print('hello')".len();
    let anchor_pos = new_content.find("[comments](#cbrust)").unwrap();
    assert!(anchor_pos > python_end, "anchor should be after the python block");
}
