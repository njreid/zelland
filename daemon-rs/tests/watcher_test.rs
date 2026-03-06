use std::fs;
use tempfile::tempdir;
use zlnd_daemon::loro_manager::LoroManager;

#[tokio::test]
async fn test_loro_reload_from_disk() {
    let _ = tracing_subscriber::fmt::try_init();

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");

    // 1. Initial file with one annotation (new format)
    let initial_content = "# Title\n\n---\n\n## Comments\n\n### ann1\n\n> some quoted text\n\n- 2026-02-11T09:00:00Z njr: First\n";
    fs::write(&file_path, initial_content).unwrap();

    let loro_manager = LoroManager::new();

    // 2. Load into Loro
    let (initial_json, mut rx) = loro_manager.subscribe(&file_path).await;
    println!("Initial JSON: {}", initial_json);
    assert!(initial_json.contains("First"));
    assert!(initial_json.contains("some quoted text"));

    // 3. Externally add a BRAND NEW annotation (new ID) to the file
    let updated_content = "# Title\n\n---\n\n## Comments\n\n### ann1\n\n> some quoted text\n\n- 2026-02-11T09:00:00Z njr: First\n\n### ann2\n\n> different quote\n\n- 2026-02-12T10:00:00Z bot: External\n";
    fs::write(&file_path, updated_content).unwrap();

    // 4. Trigger reload
    loro_manager.reload_from_disk(&file_path).await;

    // 5. Verify broadcast includes the new annotation
    let broadcast_json = rx.recv().await.unwrap();
    println!("Broadcast JSON: {}", broadcast_json);
    assert!(broadcast_json.contains("External"), "new external annotation should be broadcast");
    assert!(broadcast_json.contains("ann2"), "new annotation id should appear");

    // ann1 should still be present
    let (final_json, _) = loro_manager.subscribe(&file_path).await;
    println!("Final JSON: {}", final_json);
    assert!(final_json.contains("First"), "original annotation should remain");
    assert!(final_json.contains("External"), "new external annotation should be present");
    assert!(final_json.contains("some quoted text"), "quote should be preserved");
}
