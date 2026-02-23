use alacritty_terminal::term::{Term, Config, TermMode};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};

struct TermEventProxy;

impl EventListener for TermEventProxy {
    fn send_event(&self, _: alacritty_terminal::event::Event) {}
}

/// Minimal Dimensions impl for creating / resizing a Term.
struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize { self.rows } // No scrollback history
    fn screen_lines(&self) -> usize { self.rows }
    fn columns(&self) -> usize { self.cols }
}

pub struct TerminalSession {
    term: Term<TermEventProxy>,
    processor: Processor,
    render_buf: Vec<u8>,
    dirty: bool,
}

impl TerminalSession {
    pub fn new(cols: u16, rows: u16) -> Self {
        let config = Config {
            scrolling_history: 0, // Requested: no clientside scrollback
            ..Config::default()
        };
        let size = TermSize { cols: cols as usize, rows: rows as usize };
        let term = Term::new(config, &size, TermEventProxy);
        let processor = Processor::new();
        Self { 
            term, 
            processor,
            render_buf: Vec::with_capacity(8192),
            dirty: true,
        }
    }

    pub fn process_bytes(&mut self, data: &[u8]) {
        self.processor.advance(&mut self.term, data);
        self.dirty = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(TermSize { cols: cols as usize, rows: rows as usize });
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn get_mouse_mode(&self) -> bool {
        self.term.mode().intersects(
            TermMode::MOUSE_REPORT_CLICK | 
            TermMode::MOUSE_DRAG | 
            TermMode::MOUSE_MOTION
        )
    }

    pub fn get_cursor_pos(&self) -> (u16, u16) {
        let cursor = self.term.grid().cursor.point;
        (cursor.column.0 as u16, cursor.line.0 as u16)
    }

    /// Render the currently-visible viewport as ANSI escape sequences.
    /// Reuses an internal buffer to minimize allocations.
    pub fn render_viewport(&mut self) -> &[u8] {
        self.dirty = false;
        self.render_buf.clear();
        // SGR reset + Cursor home + erase display
        self.render_buf.extend_from_slice(b"\x1b[0m\x1b[H\x1b[2J");

        let grid = self.term.grid();
        let mut prev_fg = Color::Named(NamedColor::Foreground);
        let mut prev_bg = Color::Named(NamedColor::Background);
        let mut prev_flags = Flags::empty();
        let mut last_line = None;

        for indexed in grid.display_iter() {
            let line = indexed.point.line.0;
            let cell: &Cell = indexed.cell;

            // Explicitly move to the start of the row when the line changes.
            // This bypasses xterm.js auto-wrapping which causes extra newlines
            // when combined with explicit \r\n.
            if Some(line) != last_line {
                self.render_buf.extend_from_slice(b"\x1b[");
                push_u8(&mut self.render_buf, (line as u8).wrapping_add(1));
                self.render_buf.extend_from_slice(b";1H");
                last_line = Some(line);
            }

            // Emit SGR sequence only when attributes change
            if cell.fg != prev_fg || cell.bg != prev_bg || cell.flags != prev_flags {
                write_sgr(&mut self.render_buf, cell.fg, cell.bg, cell.flags);
                prev_fg = cell.fg;
                prev_bg = cell.bg;
                prev_flags = cell.flags;
            }

            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            let c = if cell.c == '\0' { ' ' } else { cell.c };
            let mut buf = [0u8; 4];
            self.render_buf.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }

        // Place the cursor at its actual position before sending the snapshot
        let (column, line) = self.get_cursor_pos();
        self.render_buf.extend_from_slice(b"\x1b[");
        push_u8(&mut self.render_buf, (line as u8).wrapping_add(1));
        self.render_buf.push(b';');
        push_u8(&mut self.render_buf, (column as u8).wrapping_add(1));
        self.render_buf.push(b'H');

        self.render_buf.extend_from_slice(b"\x1b[0m"); // SGR reset
        &self.render_buf
    }
}

fn write_sgr(out: &mut Vec<u8>, fg: Color, bg: Color, flags: Flags) {
    out.extend_from_slice(b"\x1b[0");

    if flags.contains(Flags::BOLD)      { out.extend_from_slice(b";1"); }
    if flags.contains(Flags::DIM)       { out.extend_from_slice(b";2"); }
    if flags.contains(Flags::ITALIC)    { out.extend_from_slice(b";3"); }
    if flags.contains(Flags::UNDERLINE) { out.extend_from_slice(b";4"); }
    if flags.contains(Flags::INVERSE)   { out.extend_from_slice(b";7"); }
    if flags.contains(Flags::HIDDEN)    { out.extend_from_slice(b";8"); }
    if flags.contains(Flags::STRIKEOUT) { out.extend_from_slice(b";9"); }

    push_fg_color(out, fg);
    push_bg_color(out, bg);

    out.push(b'm');
}

fn push_u8(out: &mut Vec<u8>, mut n: u8) {
    if n >= 100 { 
        out.push(b'0' + n / 100); 
        n %= 100;
        out.push(b'0' + n / 10);
    } else if n >= 10 {
        out.push(b'0' + n / 10);
    }
    out.push(b'0' + n % 10);
}

fn push_fg_color(out: &mut Vec<u8>, color: Color) {
    match color {
        Color::Named(n) => {
            let code = named_fg_code(n);
            if code != 39 {
                out.push(b';');
                push_u8(out, code);
            }
        }
        Color::Indexed(i) => {
            out.extend_from_slice(b";38;5;");
            push_u8(out, i);
        }
        Color::Spec(Rgb { r, g, b }) => {
            out.extend_from_slice(b";38;2;");
            push_u8(out, r); out.push(b';');
            push_u8(out, g); out.push(b';');
            push_u8(out, b);
        }
    }
}

fn push_bg_color(out: &mut Vec<u8>, color: Color) {
    match color {
        Color::Named(n) => {
            let code = named_bg_code(n);
            if code != 49 {
                out.push(b';');
                push_u8(out, code);
            }
        }
        Color::Indexed(i) => {
            out.extend_from_slice(b";48;5;");
            push_u8(out, i);
        }
        Color::Spec(Rgb { r, g, b }) => {
            out.extend_from_slice(b";48;2;");
            push_u8(out, r); out.push(b';');
            push_u8(out, g); out.push(b';');
            push_u8(out, b);
        }
    }
}

fn named_fg_code(c: NamedColor) -> u8 {
    match c {
        NamedColor::Black          => 30,
        NamedColor::Red            => 31,
        NamedColor::Green          => 32,
        NamedColor::Yellow         => 33,
        NamedColor::Blue           => 34,
        NamedColor::Magenta        => 35,
        NamedColor::Cyan           => 36,
        NamedColor::White          => 37,
        NamedColor::BrightBlack    => 90,
        NamedColor::BrightRed      => 91,
        NamedColor::BrightGreen    => 92,
        NamedColor::BrightYellow   => 93,
        NamedColor::BrightBlue     => 94,
        NamedColor::BrightMagenta  => 95,
        NamedColor::BrightCyan     => 96,
        NamedColor::BrightWhite    => 97,
        NamedColor::DimBlack       => 30,
        NamedColor::DimRed         => 31,
        NamedColor::DimGreen       => 32,
        NamedColor::DimYellow      => 33,
        NamedColor::DimBlue        => 34,
        NamedColor::DimMagenta     => 35,
        NamedColor::DimCyan        => 36,
        NamedColor::DimWhite       => 37,
        _ => 39,
    }
}

fn named_bg_code(c: NamedColor) -> u8 {
    match c {
        NamedColor::Black          => 40,
        NamedColor::Red            => 41,
        NamedColor::Green          => 42,
        NamedColor::Yellow         => 43,
        NamedColor::Blue           => 44,
        NamedColor::Magenta        => 45,
        NamedColor::Cyan           => 46,
        NamedColor::White          => 47,
        NamedColor::BrightBlack    => 100,
        NamedColor::BrightRed      => 101,
        NamedColor::BrightGreen    => 102,
        NamedColor::BrightYellow   => 103,
        NamedColor::BrightBlue     => 104,
        NamedColor::BrightMagenta  => 105,
        NamedColor::BrightCyan     => 106,
        NamedColor::BrightWhite    => 107,
        _ => 49,
    }
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
    fn test_push_u8() {
        let mut buf = Vec::new();
        push_u8(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_u8(&mut buf, 42);
        assert_eq!(buf, b"42");
        buf.clear();
        push_u8(&mut buf, 255);
        assert_eq!(buf, b"255");
    }
}
