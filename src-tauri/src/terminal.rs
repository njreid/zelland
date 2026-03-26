use crate::ghostty::*;
use log::{error, info, warn};

pub struct TerminalSession {
    term: GhosttyTerminalWrapper,
    render_state: GhosttyRenderStateWrapper,
    dirty: bool,
}

impl TerminalSession {
    pub fn new(cols: u16, rows: u16) -> Self {
        let term =
            GhosttyTerminalWrapper::new(cols, rows).expect("Failed to create Ghostty terminal");
        let render_state =
            GhosttyRenderStateWrapper::new().expect("Failed to create Ghostty render state");
        Self {
            term,
            render_state,
            dirty: true,
        }
    }

    pub fn process_bytes(&mut self, data: &[u8]) {
        self.term.write(data);
        self.dirty = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let width_px = cols as u32 * crate::renderer::CELL_WIDTH as u32;
        let height_px = rows as u32 * crate::renderer::CELL_HEIGHT as u32;
        if let Err(e) = self.term.resize(cols, rows, width_px, height_px) {
            error!("Failed to resize Ghostty terminal: {}", e);
        }
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn get_mouse_mode(&self) -> bool {
        self.term.get_mouse_tracking()
    }

    pub fn get_cursor_pos(&self) -> (u16, u16) {
        self.term.get_cursor_pos()
    }

    /// Process a mouse action and return the ANSI sequence to send to the host.
    /// Handles complex interactions like scroll wheel "clicks" (Press + Release).
    pub fn process_mouse(&self, x: f32, y: f32, action: &str) -> Vec<Vec<u8>> {
        let mut sequences = Vec::new();
        let mouse_mode = self.get_mouse_mode();
        if !mouse_mode {
            log::warn!("process_mouse: mouse_mode=false, event dropped (waiting for \\x1b[?1000h)");
            return sequences;
        }

        match action {
            "scroll_up" => {
                if let Some(press) = self.encode_mouse_event(x, y, "scroll_up") {
                    sequences.push(press);
                }
                if let Some(release) = self.encode_mouse_event(x, y, "release_scroll_up") {
                    sequences.push(release);
                }
            }
            "scroll_down" => {
                if let Some(press) = self.encode_mouse_event(x, y, "scroll_down") {
                    sequences.push(press);
                }
                if let Some(release) = self.encode_mouse_event(x, y, "release_scroll_down") {
                    sequences.push(release);
                }
            }
            "click" => {
                // Taps are sent as a Left Press then a Left Release.
                if let Some(press) = self.encode_mouse_event(x, y, "click") {
                    sequences.push(press);
                }
                if let Some(release) = self.encode_mouse_event(x, y, "release") {
                    sequences.push(release);
                }
            }
            _ => {
                if let Some(seq) = self.encode_mouse_event(x, y, action) {
                    sequences.push(seq);
                }
            }
        }

        sequences
    }

    /// Encode a mouse event as an ANSI escape sequence (SGR protocol).
    pub fn encode_mouse_event(&self, x_px: f32, y_px: f32, action: &str) -> Option<Vec<u8>> {
        let mut encoder: GhosttyMouseEncoder = std::ptr::null_mut();
        let mut event: GhosttyMouseEvent = std::ptr::null_mut();

        unsafe {
            ghostty_mouse_encoder_new(std::ptr::null(), &mut encoder);
            ghostty_mouse_encoder_setopt_from_terminal(encoder, self.term.inner);

            let (cols, rows) = self.term.get_size();
            // Use the renderer's live cell dimensions so the pixel→cell mapping
            // matches what is actually rendered. The compile-time constants are
            // only a fallback for when no renderer exists yet.
            let (cell_w, cell_h) = crate::renderer::get_cell_size();
            let size = GhosttyMouseEncoderSize {
                size: std::mem::size_of::<GhosttyMouseEncoderSize>(),
                screen_width: (cols as u32) * cell_w as u32,
                screen_height: (rows as u32) * cell_h as u32,
                cell_width: cell_w as u32,
                cell_height: cell_h as u32,
                padding_top: 0,
                padding_bottom: 0,
                padding_right: 0,
                padding_left: 0,
            };
            ghostty_mouse_encoder_setopt(
                encoder,
                GhosttyMouseEncoderOption_GHOSTTY_MOUSE_ENCODER_OPT_SIZE,
                &size as *const _ as *const std::ffi::c_void,
            );

            ghostty_mouse_event_new(std::ptr::null(), &mut event);
            ghostty_mouse_event_set_position(event, GhosttyMousePosition { x: x_px, y: y_px });

            let ghostty_action = match action {
                "click" | "scroll_up" | "scroll_down" => crate::ghostty::GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_PRESS,
                "release" | "release_scroll_up" | "release_scroll_down" => crate::ghostty::GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_RELEASE,
                _ => crate::ghostty::GhosttyMouseAction_GHOSTTY_MOUSE_ACTION_PRESS,
            };
            ghostty_mouse_event_set_action(event, ghostty_action);

            let button = match action {
                "click" | "release" => crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_LEFT,
                "right_click" => crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_RIGHT,
                "scroll_up" | "release_scroll_up" => crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_FOUR,
                "scroll_down" | "release_scroll_down" => crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_FIVE,
                _ => crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_UNKNOWN,
            };
            if button != crate::ghostty::GhosttyMouseButton_GHOSTTY_MOUSE_BUTTON_UNKNOWN {
                ghostty_mouse_event_set_button(event, button);
            } else {
                ghostty_mouse_event_clear_button(event);
            }

            ghostty_mouse_event_set_mods(event, 0);
        }

        let mut buf = [0u8; 64];
        let mut out_len: usize = 0;
        let res = unsafe {
            ghostty_mouse_encoder_encode(
                encoder,
                event,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                &mut out_len,
            )
        };

        unsafe {
            ghostty_mouse_event_free(event);
            ghostty_mouse_encoder_free(encoder);
        }

        if res == GhosttyResult_GHOSTTY_SUCCESS && out_len > 0 {
            Some(buf[..out_len].to_vec())
        } else {
            log::warn!("encode_mouse_event: encoder returned nothing (res={} action={})", res, action);
            None
        }
    }

    /// Render using the native wgpu renderer. Returns an error if the render
    /// state update fails; a missing renderer (surface not yet created or
    /// destroyed on screen-lock) is silently skipped.
    pub fn render_native(&mut self) -> Result<(), String> {
        self.render_state
            .update(&self.term)
            .map_err(|e| format!("Failed to update render state: {}", e))?;

        let cursor_pos = self.term.get_cursor_pos();
        let had_renderer = crate::renderer::with_renderer(|renderer| {
            renderer.draw_ghostty_state(&mut self.render_state, cursor_pos);
            renderer.render();
        }).is_some();

        if had_renderer {
            // Only clear dirty flags after a frame was actually submitted.
            self.dirty = false;
            self.render_state.reset_dirty();
        } else {
            warn!("render_native: no renderer available (surface not yet ready)");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_session_processes_data() {
        let mut ts = TerminalSession::new(80, 24);
        assert!(ts.is_dirty(), "new session should be dirty");
        ts.process_bytes(b"hello");
        assert!(ts.is_dirty(), "should be dirty after process_bytes");
        let (col, row) = ts.get_cursor_pos();
        assert!(col < 80 && row < 24, "cursor should be within terminal bounds");
    }

    #[test]
    fn test_terminal_session_render_native_without_renderer() {
        let mut ts = TerminalSession::new(80, 24);
        ts.process_bytes(b"test");
        // render_native must not panic when no renderer surface exists
        assert!(ts.render_native().is_ok());
    }

    #[test]
    fn test_encode_mouse_event_uses_sgr_sequences_when_enabled() {
        let ts = TerminalSession::new(80, 24);
        ts.term.write(b"\x1b[?1000h\x1b[?1006h");

        let encoded = ts
            .encode_mouse_event(48.0, 64.0, "click")
            .expect("mouse event should encode");
        let text = String::from_utf8(encoded).expect("encoded mouse sequence should be utf8");

        assert!(text.starts_with("\x1b[<"));
        assert!(text.ends_with('M'));
    }

    #[test]
    fn test_encode_mouse_event_supports_right_click() {
        let ts = TerminalSession::new(80, 24);
        ts.term.write(b"\x1b[?1000h\x1b[?1006h");

        let encoded = ts
            .encode_mouse_event(24.0, 32.0, "right_click")
            .expect("mouse event should encode");
        let text = String::from_utf8(encoded).expect("encoded mouse sequence should be utf8");

        assert!(text.starts_with("\x1b[<"));
        assert!(text.contains(';'));
    }
}
