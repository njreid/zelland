use crate::ghostty::*;

pub struct TerminalSession {
    term: GhosttyTerminalWrapper,
    render_state: GhosttyRenderStateWrapper,
    render_buf: Vec<u8>,
    dirty: bool,
}

impl TerminalSession {
    pub fn new(cols: u16, rows: u16) -> Self {
        let term = GhosttyTerminalWrapper::new(cols, rows).expect("Failed to create Ghostty terminal");
        let render_state = GhosttyRenderStateWrapper::new().expect("Failed to create Ghostty render state");
        
        Self { 
            term, 
            render_state,
            render_buf: Vec::with_capacity(8192),
            dirty: true,
        }
    }

    pub fn process_bytes(&mut self, data: &[u8]) {
        self.term.write(data);
        self.dirty = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        // Pixel sizes are currently approximated or zeroed since we're in Phase 1 (JS rendering)
        self.term.resize(cols, rows, 0, 0).expect("Failed to resize Ghostty terminal");
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

    /// Encode a mouse event as an ANSI escape sequence (SGR protocol).
    pub fn encode_mouse_event(&self, x_px: f32, y_px: f32, action: &str) -> Option<Vec<u8>> {
        let mut encoder: GhosttyMouseEncoder = std::ptr::null_mut();
        let mut event: GhosttyMouseEvent = std::ptr::null_mut();

        unsafe {
            ghostty_mouse_encoder_new(std::ptr::null(), &mut encoder);
            ghostty_mouse_encoder_setopt_from_terminal(encoder, self.term.inner);
            
            ghostty_mouse_event_new(std::ptr::null(), &mut event);
            
            ghostty_mouse_event_set_position(event, GhosttyMousePosition { x: x_px, y: y_px });
            
            let ghostty_action = match action {
                "click" => GHOSTTY_MOUSE_ACTION_PRESS,
                "release" => GHOSTTY_MOUSE_ACTION_RELEASE,
                _ => GHOSTTY_MOUSE_ACTION_PRESS,
            };
            ghostty_mouse_event_set_action(event, ghostty_action);

            let button = match action {
                "click" | "release" => GHOSTTY_MOUSE_BUTTON_LEFT,
                "right_click" => GHOSTTY_MOUSE_BUTTON_RIGHT,
                _ => GHOSTTY_MOUSE_BUTTON_UNKNOWN,
            };
            if button != GHOSTTY_MOUSE_BUTTON_UNKNOWN {
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
                buf.as_mut_ptr() as *mut i8,
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
            None
        }
    }

    /// Render using the native wgpu renderer if available.
    pub fn render_native(&mut self) {
        self.dirty = false;
        self.render_state.update(&self.term).expect("Failed to update render state");
        
        crate::renderer::with_renderer(|renderer| {
            renderer.draw_ghostty_state(&mut self.render_state);
            renderer.render();
        });

        self.render_state.reset_dirty();
    }

    /// Render the currently-visible viewport as ANSI escape sequences.
    /// Reuses an internal buffer to minimize allocations.
    pub fn render_viewport(&mut self) -> &[u8] {
        self.dirty = false;
        self.render_state.update(&self.term).expect("Failed to update render state");
        
        self.render_buf.clear();
        // SGR reset + Cursor home + erase display
        self.render_buf.extend_from_slice(b"\x1b[0m\x1b[H\x1b[2J");

        let mut prev_style: Option<GhosttyStyle> = None;
        let mut last_line: Option<u16> = None;

        // Ghostty uses a row iterator
        self.render_state.with_rows(|line, cells| {
            // Move to start of line
            if Some(line) != last_line {
                self.render_buf.extend_from_slice(b"\x1b[");
                push_u16(&mut self.render_buf, line.wrapping_add(1));
                self.render_buf.extend_from_slice(b";1H");
                last_line = Some(line);
            }

            while unsafe { ghostty_render_state_row_cells_next(*cells) } {
                let style = get_cell_style(*cells);
                
                // Emit SGR sequence only when attributes change
                if prev_style.as_ref().map_or(true, |p| !styles_equal(p, &style)) {
                    write_sgr_ghostty(&mut self.render_buf, &style);
                    prev_style = Some(style);
                }

                let graphemes = get_cell_graphemes(*cells);
                if graphemes.is_empty() {
                    self.render_buf.push(b' ');
                } else {
                    for &cp in &graphemes {
                        if let Some(c) = std::char::from_u32(cp) {
                            let mut buf = [0u8; 4];
                            self.render_buf.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                }
            }
        });

        // Place the cursor at its actual position
        let (column, line) = self.get_cursor_pos();
        self.render_buf.extend_from_slice(b"\x1b[");
        push_u16(&mut self.render_buf, line.wrapping_add(1));
        self.render_buf.push(b';');
        push_u16(&mut self.render_buf, column.wrapping_add(1));
        self.render_buf.push(b'H');

        self.render_buf.extend_from_slice(b"\x1b[0m"); // SGR reset
        self.render_state.reset_dirty();
        
        &self.render_buf
    }
}

fn styles_equal(a: &GhosttyStyle, b: &GhosttyStyle) -> bool {
    a.bold == b.bold &&
    a.italic == b.italic &&
    a.faint == b.faint &&
    a.blink == b.blink &&
    a.inverse == b.inverse &&
    a.invisible == b.invisible &&
    a.strikethrough == b.strikethrough &&
    a.overline == b.overline &&
    a.underline == b.underline &&
    colors_equal(&a.fg_color, &b.fg_color) &&
    colors_equal(&a.bg_color, &b.bg_color)
}

#[allow(non_upper_case_globals)]
fn colors_equal(a: &GhosttyStyleColor, b: &GhosttyStyleColor) -> bool {
    if a.tag != b.tag { return false; }
    match a.tag {
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_NONE => true,
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_PALETTE => unsafe { a.value.palette == b.value.palette },
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_RGB => unsafe { 
            a.value.rgb.r == b.value.rgb.r && 
            a.value.rgb.g == b.value.rgb.g && 
            a.value.rgb.b == b.value.rgb.b 
        },
        _ => true,
    }
}

#[allow(non_upper_case_globals)]
fn write_sgr_ghostty(out: &mut Vec<u8>, style: &GhosttyStyle) {
    out.extend_from_slice(b"\x1b[0");

    if style.bold          { out.extend_from_slice(b";1"); }
    if style.faint         { out.extend_from_slice(b";2"); }
    if style.italic        { out.extend_from_slice(b";3"); }
    // Ghostty underline is an int (None=0, Single=1, etc.)
    if style.underline > 0 { out.extend_from_slice(b";4"); }
    if style.inverse       { out.extend_from_slice(b";7"); }
    if style.invisible     { out.extend_from_slice(b";8"); }
    if style.strikethrough { out.extend_from_slice(b";9"); }

    push_ghostty_color(out, &style.fg_color, false);
    push_ghostty_color(out, &style.bg_color, true);

    out.push(b'm');
}

#[allow(non_upper_case_globals)]
fn push_ghostty_color(out: &mut Vec<u8>, color: &GhosttyStyleColor, is_bg: bool) {
    let prefix = if is_bg { b";48" } else { b";38" };
    match color.tag {
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_NONE => {},
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_PALETTE => {
            out.extend_from_slice(prefix);
            out.extend_from_slice(b";5;");
            push_u16(out, unsafe { color.value.palette } as u16);
        }
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_RGB => {
            out.extend_from_slice(prefix);
            out.extend_from_slice(b";2;");
            let rgb = unsafe { color.value.rgb };
            push_u16(out, rgb.r as u16); out.push(b';');
            push_u16(out, rgb.g as u16); out.push(b';');
            push_u16(out, rgb.b as u16);
        }
        _ => {}
    }
}

fn push_u16(out: &mut Vec<u8>, mut n: u16) {
    if n >= 10000 {
        out.push(b'0' + (n / 10000) as u8);
        n %= 10000;
        out.push(b'0' + (n / 1000) as u8);
        n %= 1000;
        out.push(b'0' + (n / 100) as u8);
        n %= 100;
        out.push(b'0' + (n / 10) as u8);
    } else if n >= 1000 {
        out.push(b'0' + (n / 1000) as u8);
        n %= 1000;
        out.push(b'0' + (n / 100) as u8);
        n %= 100;
        out.push(b'0' + (n / 10) as u8);
    } else if n >= 100 { 
        out.push(b'0' + (n / 100) as u8); 
        n %= 100;
        out.push(b'0' + (n / 10) as u8);
    } else if n >= 10 {
        out.push(b'0' + (n / 10) as u8);
    }
    out.push(b'0' + (n % 10) as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_session_no_scrollback() {
        let mut ts = TerminalSession::new(80, 24);
        ts.process_bytes(b"Line 1\r\nLine 2");
        let viewport = String::from_utf8_lossy(ts.render_viewport());
        assert!(viewport.contains("Line 1"));
        assert!(viewport.contains("Line 2"));
        // Check for absolute positioning sequence for line 2 (starts at row 2)
        assert!(viewport.contains("\x1b[2;1H"));
    }

    #[test]
    fn test_push_u16() {
        let mut buf = Vec::new();
        push_u16(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_u16(&mut buf, 42);
        assert_eq!(buf, b"42");
        buf.clear();
        push_u16(&mut buf, 255);
        assert_eq!(buf, b"255");
        buf.clear();
        push_u16(&mut buf, 1234);
        assert_eq!(buf, b"1234");
    }
}
