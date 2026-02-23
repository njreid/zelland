use std::fs;
use crate::store::{parse_markdown_comments, reify_markdown_comments};
use crate::handlers::annotations::Comment;
use tempfile::tempdir;

#[test]
fn test_markdown_comments_roundtrip() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    
    let initial_content = "# Title\n\nSome content [here](#ann1).\n";
    fs::write(&file_path, initial_content).unwrap();

    let anns = vec![
        ("ann1".to_string(), vec![
            Comment {
                author: "njr".to_string(),
                timestamp: "2026-02-11T09:00:00Z".to_string(),
                body: "First comment".to_string(),
            },
            Comment {
                author: "bot".to_string(),
                timestamp: "2026-02-12T09:00:00Z".to_string(),
                body: "Second comment".to_string(),
            },
        ]),
    ];

    reify_markdown_comments(&file_path, &anns).unwrap();
    
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("# Comments"));
    assert!(content.contains("## ann1"));
    assert!(content.contains("- 2026-02-11T09:00:00Z njr: First comment"));

    let parsed = parse_markdown_comments(&file_path).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, "ann1");
    assert_eq!(parsed[0].1.len(), 2);
    assert_eq!(parsed[0].1[0].author, "njr");
    assert_eq!(parsed[0].1[1].body, "Second comment");
}

#[test]
fn test_reify_overwrites_existing_comments() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    
    let content = "# Title\n\nContent\n\n# Comments\n\n## old\n- 2026-01-01T00:00:00Z anon: old\n";
    fs::write(&file_path, content).unwrap();

    let new_anns = vec![
        ("new".to_string(), vec![
            Comment {
                author: "user".to_string(),
                timestamp: "2026-02-23T00:00:00Z".to_string(),
                body: "new comment".to_string(),
            },
        ]),
    ];

    reify_markdown_comments(&file_path, &new_anns).unwrap();
    
    let final_content = fs::read_to_string(&file_path).unwrap();
    assert!(!final_content.contains("## old"));
    assert!(final_content.contains("## new"));
}
