/// An in-memory terminal emulator backed by a `vt100::Parser`.
///
/// Parses ANSI/VT escape sequences from child process output and maintains
/// a cell grid supporting 256-color and 24-bit RGB with full attribute
/// tracking (bold, dim, italic, underline, inverse, wide chars).
///
/// Intercepts standard terminal query sequences (`\e[6n`, `\e[18t`) and
/// generates appropriate responses so that child processes (htop, btop, etc.)
/// can discover the real terminal dimensions.
pub struct Terminal {
    parser: vt100::Parser,
    responses: Vec<Vec<u8>>,
    g0_dec_special_graphics: bool,
    g1_dec_special_graphics: bool,
    using_g1_charset: bool,
    escape_state: EscapeState,
    application_cursor_keys: bool,
}

#[derive(Clone, Copy)]
enum EscapeState {
    Ground,
    Escape,
    CharsetSelect(u8),
    Csi,
    String,
    StringEscape,
}

impl Terminal {
    /// Create a new terminal of the given dimensions (rows × cols).
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            responses: Vec::new(),
            g0_dec_special_graphics: false,
            g1_dec_special_graphics: false,
            using_g1_charset: false,
            escape_state: EscapeState::Ground,
            application_cursor_keys: false,
        }
    }

    /// Feed raw bytes from the child process into the ANSI parser.
    ///
    /// Standard terminal query sequences are intercepted and responses are
    /// queued.  Call [`drain_responses`] after processing to retrieve them.
    pub fn process(&mut self, bytes: &[u8]) {
        if bytes.contains(&0x1b) {
            self.handle_queries(bytes);
            self.handle_cursor_mode(bytes);
        }
        let translated = self.translate_dec_special_graphics(bytes);
        self.parser.process(&translated);
    }

    /// Return any queued response bytes (cursor-position reports, window-size
    /// reports, etc.) and clear the queue.
    pub fn drain_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.responses)
    }

    pub fn application_cursor_keys(&self) -> bool {
        self.application_cursor_keys
    }

    fn handle_cursor_mode(&mut self, bytes: &[u8]) {
        if bytes.windows(5).any(|w| w == b"\x1b[?1h") {
            self.application_cursor_keys = true;
        }
        if bytes.windows(5).any(|w| w == b"\x1b[?1l") {
            self.application_cursor_keys = false;
        }
    }

    /// Resize the terminal grid after a window resize.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
    }

    /// Return a reference to the cell grid for rendering.
    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Convenience: return `(rows, cols)` of the terminal grid.
    #[allow(dead_code)]
    pub fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    fn active_dec_special_graphics(&self) -> bool {
        if self.using_g1_charset {
            self.g1_dec_special_graphics
        } else {
            self.g0_dec_special_graphics
        }
    }

    fn translate_dec_special_graphics(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut translated = Vec::with_capacity(bytes.len());

        for &byte in bytes {
            match self.escape_state {
                EscapeState::Ground => match byte {
                    0x1b => {
                        self.escape_state = EscapeState::Escape;
                    }
                    0x0e => {
                        self.using_g1_charset = true;
                    }
                    0x0f => {
                        self.using_g1_charset = false;
                    }
                    0x20..=0x7e if self.active_dec_special_graphics() => {
                        push_dec_special_graphic(&mut translated, byte);
                    }
                    _ => {
                        translated.push(byte);
                    }
                },
                EscapeState::Escape => match byte {
                    b'(' | b')' => {
                        self.escape_state = EscapeState::CharsetSelect(byte);
                    }
                    b'[' => {
                        self.escape_state = EscapeState::Csi;
                        translated.push(0x1b);
                        translated.push(byte);
                    }
                    b']' | b'P' | b'^' | b'_' => {
                        self.escape_state = EscapeState::String;
                        translated.push(0x1b);
                        translated.push(byte);
                    }
                    _ => {
                        self.escape_state = EscapeState::Ground;
                        translated.push(0x1b);
                        translated.push(byte);
                    }
                },
                EscapeState::CharsetSelect(charset) => {
                    match charset {
                        b'(' => self.g0_dec_special_graphics = byte == b'0',
                        b')' => self.g1_dec_special_graphics = byte == b'0',
                        _ => {}
                    }
                    self.escape_state = EscapeState::Ground;
                }
                EscapeState::Csi => {
                    translated.push(byte);
                    if (0x40..=0x7e).contains(&byte) {
                        self.escape_state = EscapeState::Ground;
                    }
                }
                EscapeState::String => {
                    translated.push(byte);
                    match byte {
                        0x07 => self.escape_state = EscapeState::Ground,
                        0x1b => self.escape_state = EscapeState::StringEscape,
                        _ => {}
                    }
                }
                EscapeState::StringEscape => {
                    translated.push(byte);
                    self.escape_state = if byte == b'\\' {
                        EscapeState::Ground
                    } else {
                        EscapeState::String
                    };
                }
            }
        }

        translated
    }

    fn handle_queries(&mut self, bytes: &[u8]) {
        if bytes.windows(4).any(|w| w == b"\x1b[6n") {
            let (cur_row, cur_col) = self.parser.screen().cursor_position();
            self.responses.push(format!("\x1b[{};{}R", cur_row + 1, cur_col + 1).into_bytes());
        }
        if bytes.windows(5).any(|w| w == b"\x1b[18t") {
            let (rows, cols) = self.parser.screen().size();
            self.responses.push(format!("\x1b[8;{};{}t", rows, cols).into_bytes());
        }
    }
}

fn push_dec_special_graphic(out: &mut Vec<u8>, byte: u8) {
    let mapped = match byte {
        b'`' => '◆',
        b'a' => '▒',
        b'b' => '␉',
        b'c' => '␌',
        b'd' => '␍',
        b'e' => '␊',
        b'f' => '°',
        b'g' => '±',
        b'h' => '␤',
        b'i' => '␋',
        b'j' => '┘',
        b'k' => '┐',
        b'l' => '┌',
        b'm' => '└',
        b'n' => '┼',
        b'o' => '⎺',
        b'p' => '⎻',
        b'q' => '─',
        b'r' => '⎼',
        b's' => '⎽',
        b't' => '├',
        b'u' => '┤',
        b'v' => '┴',
        b'w' => '┬',
        b'x' => '│',
        b'y' => '≤',
        b'z' => '≥',
        b'{' => 'π',
        b'|' => '≠',
        b'}' => '£',
        b'~' => '·',
        _ => {
            out.push(byte);
            return;
        }
    };

    let mut buf = [0u8; 4];
    out.extend_from_slice(mapped.encode_utf8(&mut buf).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::Terminal;

    #[test]
    fn translates_g0_dec_vertical_line() {
        let mut terminal = Terminal::new(1, 3);

        terminal.process(b"A\x1b(0x\x1b(BB");

        assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
        assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
        assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
    }

    #[test]
    fn translates_g1_dec_vertical_line() {
        let mut terminal = Terminal::new(1, 3);

        terminal.process(b"A\x1b)0\x0ex\x0fB");

        assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
        assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
        assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
    }

    #[test]
    fn preserves_utf8_vertical_line() {
        let mut terminal = Terminal::new(1, 3);

        terminal.process("A│B".as_bytes());

        assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
        assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
        assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
    }

    #[test]
    fn does_not_translate_csi_while_dec_graphics_is_active() {
        let mut terminal = Terminal::new(1, 2);

        terminal.process(b"\x1b(0\x1b[31mx");

        let cell = terminal.screen().cell(0, 0).unwrap();
        assert_eq!(cell.contents(), "│");
        assert_eq!(cell.fgcolor(), vt100::Color::Idx(1));
    }
}
