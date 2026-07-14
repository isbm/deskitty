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
    dec_special_graphics: bool,
    pending_escape: bool,
    pending_charset_select: bool,
}

impl Terminal {
    /// Create a new terminal of the given dimensions (rows × cols).
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            responses: Vec::new(),
            dec_special_graphics: false,
            pending_escape: false,
            pending_charset_select: false,
        }
    }

    /// Feed raw bytes from the child process into the ANSI parser.
    ///
    /// Standard terminal query sequences are intercepted and responses are
    /// queued.  Call [`drain_responses`] after processing to retrieve them.
    pub fn process(&mut self, bytes: &[u8]) {
        if bytes.contains(&0x1b) {
            self.handle_queries(bytes);
        }
        let translated = self.translate_dec_special_graphics(bytes);
        self.parser.process(&translated);
    }

    /// Return any queued response bytes (cursor-position reports, window-size
    /// reports, etc.) and clear the queue.
    pub fn drain_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.responses)
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

    fn translate_dec_special_graphics(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut translated = Vec::with_capacity(bytes.len());

        for &byte in bytes {
            if self.pending_charset_select {
                self.pending_charset_select = false;
                self.pending_escape = false;
                self.dec_special_graphics = byte == b'0';
                continue;
            }

            if self.pending_escape {
                self.pending_escape = false;
                if byte == b'(' {
                    self.pending_charset_select = true;
                    continue;
                }
                translated.push(0x1b);
                translated.push(byte);
                continue;
            }

            if byte == 0x1b {
                self.pending_escape = true;
                continue;
            }

            if self.dec_special_graphics {
                push_dec_special_graphic(&mut translated, byte);
            } else {
                translated.push(byte);
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
