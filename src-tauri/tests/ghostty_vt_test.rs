use tauri_app_lib::ghostty::GhosttyTerminalWrapper;

unsafe extern "C" {
    fn ghostty_android_tls_fix();
}

#[test]
fn test_ghostty_terminal_init() {
    unsafe { ghostty_android_tls_fix(); }
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create Ghostty terminal");
    let (cols, rows) = term.get_size();
    assert_eq!(cols, 80);
    assert_eq!(rows, 24);
}

#[test]
fn test_ghostty_terminal_write_and_cursor() {
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create Ghostty terminal");
    term.write(b"Hello");
    let (x, y) = term.get_cursor_pos();
    assert_eq!(x, 5);
    assert_eq!(y, 0);
}

#[test]
fn test_ghostty_terminal_resize() {
    let term = GhosttyTerminalWrapper::new(80, 24).expect("Failed to create Ghostty terminal");
    term.resize(100, 30, 1000, 600).expect("Failed to resize Ghostty terminal");
    let (cols, rows) = term.get_size();
    assert_eq!(cols, 100);
    assert_eq!(rows, 30);
}
