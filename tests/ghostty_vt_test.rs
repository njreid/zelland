use tauri_app_lib::ghostty::*;

#[test]
fn test_ghostty_new_and_write() {
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create terminal");
    term.write(b"Hello from Ghostty!");
    
    let (cols, rows) = term.get_size();
    assert_eq!(cols, 80);
    assert_eq!(rows, 24);

    let (x, y) = term.get_cursor_pos();
    assert_eq!(x, 19); // "Hello from Ghostty!" is 19 chars
    assert_eq!(y, 0);
}

#[test]
fn test_ghostty_render_state() {
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create terminal");
    let mut state = GhosttyRenderStateWrapper::new().expect("Failed to create render state");
    
    term.write(b"Ghostty Render Test");
    state.update(&term).expect("Failed to update state");
    
    let mut found = false;
    state.with_rows(|line, cells| {
        if line == 0 {
            let graphemes = get_cell_graphemes(*cells);
            if !graphemes.is_empty() && graphemes[0] == 'G' as u32 {
                found = true;
            }
        }
    });
    
    assert!(found, "Did not find 'G' at (0,0)");
}

#[test]
fn test_ghostty_resize() {
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create terminal");
    term.resize(100, 30, 0, 0).expect("Failed to resize");
    
    let (cols, rows) = term.get_size();
    assert_eq!(cols, 100);
    assert_eq!(rows, 30);
}
