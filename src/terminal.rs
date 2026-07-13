/// An in-memory terminal emulator backed by a `vt100::Parser`.
///
/// Parses ANSI/VT escape sequences from child process output and maintains
/// a cell grid supporting 256-color and 24-bit RGB with full attribute
/// tracking (bold, dim, italic, underline, inverse, wide chars).
pub struct Terminal {
    parser: vt100::Parser,
}

impl Terminal {
    /// Create a new terminal of the given dimensions (rows × cols).
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { parser: vt100::Parser::new(rows, cols, 0) }
    }

    /// Feed raw bytes from the child process into the ANSI parser.
    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
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
}
