use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// An entry in the command-mode popup menu.
pub enum MenuItem {
    /// Switch to a tab.
    Tab { idx: usize, shortcut: char, name: String },
    /// Quit deskitty entirely.
    Quit,
}

/// The command-mode popup menu state.
pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected: usize,
}

impl MenuState {
    /// Build a menu from tab labels. A `Quit` entry is always appended.
    pub fn new(labels: &[(char, String)]) -> Self {
        let mut items: Vec<MenuItem> =
            labels.iter().enumerate().map(|(idx, (shortcut, name))| MenuItem::Tab { idx, shortcut: *shortcut, name: name.clone() }).collect();
        items.push(MenuItem::Quit);
        Self { items, selected: 0 }
    }

    /// Move selection one item down (wrapping).
    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
    }

    /// Move selection one item up (wrapping).
    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Find the index of the item matching the given shortcut character.
    ///
    /// Returns `None` if no item matches.
    pub fn find_by_shortcut(&self, c: char) -> Option<usize> {
        self.items.iter().position(|item| match item {
            MenuItem::Tab { shortcut, .. } => *shortcut == c,
            MenuItem::Quit => c == 'q',
        })
    }
}

/// Current interaction mode.
pub enum Mode {
    /// Keys are forwarded to the active tab's PTY.
    Normal,
    /// A popup menu is displayed; keys navigate the menu.
    Command(MenuState),
}

/// Render the full frame: terminal grid, status bar, and optional command menu.
pub fn render(f: &mut Frame, screen: &vt100::Screen, labels: &[(char, String)], active_idx: usize, mode: &Mode) {
    let area = f.area();

    let layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(area);

    let grid_area = layout[0];
    let status_area = layout[1];

    render_terminal_grid(f, grid_area, screen);
    render_status_bar(f, status_area, labels, active_idx, mode);

    if let Mode::Command(menu) = mode {
        render_menu(f, area, menu);
    }
}

fn render_terminal_grid(f: &mut Frame, area: Rect, screen: &vt100::Screen) {
    let (rows, cols) = screen.size();
    let max_rows = area.height.min(rows);
    let max_cols = area.width.min(cols);

    let mut lines = Vec::new();
    for row in 0..max_rows {
        let mut spans: Vec<Span> = Vec::new();
        let mut col_offset: u16 = 0;
        while col_offset < max_cols {
            if let Some(cell) = screen.cell(row, col_offset) {
                if cell.is_wide_continuation() {
                    col_offset += 1;
                    continue;
                }
                let span = cell_to_span(cell);
                let width = if cell.is_wide() { 2 } else { 1 };
                spans.push(span);
                col_offset += width;
            } else {
                spans.push(Span::raw(" "));
                col_offset += 1;
            }
        }
        lines.push(Line::from(spans));
    }

    let text = Text::from(lines);
    f.render_widget(Paragraph::new(text), area);

    let (cursor_row, cursor_col) = screen.cursor_position();
    if cursor_row < max_rows && cursor_col < max_cols {
        f.set_cursor_position((area.x + cursor_col, area.y + cursor_row));
    }
}

fn render_status_bar(f: &mut Frame, area: Rect, labels: &[(char, String)], active_idx: usize, mode: &Mode) {
    let (bg, base_fg) = match mode {
        Mode::Normal => (Color::Blue, Color::White),
        Mode::Command(_) => (Color::Yellow, Color::Black),
    };

    let mut spans: Vec<Span> = Vec::new();
    for (i, (shortcut, name)) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(base_fg)));
        }
        let text = if i == active_idx { format!("*{}:{}*", shortcut, name) } else { format!("{}:{}", shortcut, name) };
        let style = Style::default().fg(base_fg).add_modifier(if i == active_idx { Modifier::BOLD } else { Modifier::empty() });
        spans.push(Span::styled(text, style));
    }

    spans.push(Span::raw("  "));

    let hint = match mode {
        Mode::Normal => "Ctrl+\\ Menu",
        Mode::Command(_) => "Esc=Close  q=Quit",
    };
    spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));

    let line = Line::from(spans).style(Style::default().bg(bg));
    f.render_widget(Paragraph::new(line), area);
}

fn render_menu(f: &mut Frame, area: Rect, menu: &MenuState) {
    let menu_width: u16 = 24;
    let menu_height = (menu.items.len() + 2) as u16;

    let x = area.x + (area.width.saturating_sub(menu_width)) / 2;
    let y = area.y + (area.height.saturating_sub(menu_height)) / 2;
    let menu_area = Rect::new(x, y, menu_width, menu_height);

    f.render_widget(Clear, menu_area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, item) in menu.items.iter().enumerate() {
        let (shortcut_text, label) = match item {
            MenuItem::Tab { shortcut, name, .. } => (shortcut.to_string(), name.clone()),
            MenuItem::Quit => ("q".to_string(), "Quit".to_string()),
        };
        let indicator = if i == menu.selected { ">" } else { " " };
        let text = format!("{indicator} {shortcut_text}  {label}");

        let style =
            if i == menu.selected { Style::default().fg(Color::Black).bg(Color::White) } else { Style::default().fg(Color::White).bg(Color::Black) };
        lines.push(Line::from(Span::styled(text, style)));
    }

    let text = Text::from(lines);
    let block = Block::default().borders(Borders::ALL).title(" Tab Menu ");
    let inner_area = block.inner(menu_area);
    f.render_widget(block, menu_area);
    f.render_widget(Paragraph::new(text), inner_area);
}

fn cell_to_span(cell: &vt100::Cell) -> Span<'_> {
    let fg = to_ratatui_color(cell.fgcolor());
    let bg = to_ratatui_color(cell.bgcolor());

    let mut style = Style::default().fg(fg).bg(bg);

    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.dim() {
        style = style.add_modifier(Modifier::DIM);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    Span::styled(cell.contents().to_string(), style)
}

fn to_ratatui_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Convert a crossterm key event into the raw bytes to write to the PTY.
///
/// Returns `None` if the key cannot be represented (e.g. unsupported modifier
/// combinations).
pub fn key_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if c.is_ascii_alphabetic() { Some(vec![(c.to_ascii_lowercase() as u8) & 0x1f]) } else { None }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                let mut v = vec![0x1b];
                let mut buf = [0u8; 4];
                let len = c.encode_utf8(&mut buf).len();
                v.extend_from_slice(&buf[..len]);
                Some(v)
            } else {
                let mut buf = [0u8; 4];
                let len = c.encode_utf8(&mut buf).len();
                Some(buf[..len].to_vec())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => fn_key(n),
        _ => None,
    }
}

fn fn_key(n: u8) -> Option<Vec<u8>> {
    match n {
        1 => Some(b"\x1bOP".to_vec()),
        2 => Some(b"\x1bOQ".to_vec()),
        3 => Some(b"\x1bOR".to_vec()),
        4 => Some(b"\x1bOS".to_vec()),
        5 => Some(b"\x1b[15~".to_vec()),
        6 => Some(b"\x1b[17~".to_vec()),
        7 => Some(b"\x1b[18~".to_vec()),
        8 => Some(b"\x1b[19~".to_vec()),
        9 => Some(b"\x1b[20~".to_vec()),
        10 => Some(b"\x1b[21~".to_vec()),
        11 => Some(b"\x1b[23~".to_vec()),
        12 => Some(b"\x1b[24~".to_vec()),
        _ => None,
    }
}
