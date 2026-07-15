use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui_glamour::color::{blend_1d, blend_2d, lerp_color};
use ratatui_glamour::rule::dashed_title;
use ratatui_glamour::surface::gradient_rounded_panel_lines;

use crate::sysinfo::SysInfo;

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

struct GradientRect {
    colors: Vec<Color>,
    width: u16,
}

impl Widget for &GradientRect {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let local_x = (x - area.x) as usize;
                let local_y = (y - area.y) as usize;
                let idx = local_y * self.width as usize + local_x;
                if let Some(&color) = self.colors.get(idx)
                    && let Some(cell) = buf.cell_mut((x, y))
                {
                    cell.reset();
                    cell.set_bg(color);
                }
            }
        }
    }
}

struct MenuShadow {
    area: Rect,
}

impl Widget for &MenuShadow {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        let shadow_bg = crate::palette::SHADOW_BG;
        let shadow_fg = crate::palette::SHADOW_FG;
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        let x = self.area.x;
        let y = self.area.y;
        let w = self.area.width;
        let h = self.area.height;

        for idx in 0..w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(h);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut((sx, sy)) {
                cell.set_bg(shadow_bg);
                cell.set_fg(shadow_fg);
            }
        }

        for offset in 0..2u16 {
            for idx in 0..h {
                let sx = x.saturating_add(w).saturating_add(offset);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > max_x || sy > max_y {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((sx, sy)) {
                    cell.set_bg(shadow_bg);
                    cell.set_fg(shadow_fg);
                }
            }
        }
    }
}

struct DashedTitle<'a> {
    text: &'a str,
    text_fg: Color,
    grad_start: Color,
    grad_end: Color,
}

impl Widget for &DashedTitle<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        dashed_title(area, buf, self.text, self.text_fg, self.grad_start, self.grad_end);
    }
}

struct GradientPanel {
    stops: Vec<Color>,
}

impl Widget for &GradientPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = gradient_rounded_panel_lines(area.width as usize, area.height as usize, Style::default(), &self.stops);
        for (row, line) in lines.into_iter().enumerate() {
            buf.set_line(area.x, area.y + row as u16, &line, area.width);
        }
    }
}

fn bottom_right_corner_color(area: Rect, stops: &[Color]) -> Color {
    let w = area.width as usize;
    let h = area.height as usize;
    let perimeter_len = 2 * w + 2 * h - 4;
    let corner_idx = w + h - 2;
    if perimeter_len == 0 || stops.is_empty() {
        return Color::Reset;
    }
    let pos = corner_idx as f32 / (perimeter_len - 1) as f32;
    let segments = stops.len() - 1;
    let scaled = pos * segments as f32;
    let base = scaled.floor() as usize;
    if base >= segments {
        stops[segments]
    } else {
        lerp_color(stops[base], stops[base + 1], scaled - base as f32)
    }
}

/// Render the full frame: terminal grid, status bar, and optional command menu.
pub fn render(f: &mut Frame, screen: &vt100::Screen, labels: &[(char, String)], active_idx: usize, mode: &Mode, sysinfo: Option<&SysInfo>, bg_active: bool) {
    let area = f.area();

    let border_stops =
        vec![Color::Rgb(255, 95, 135), Color::Rgb(135, 95, 255), Color::Rgb(0, 255, 255), Color::Rgb(135, 95, 255), Color::Rgb(255, 95, 135)];
    let corner_color = bottom_right_corner_color(area, &border_stops);
    f.render_widget(&GradientPanel { stops: border_stops }, area);

    let inner = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(2));
    let grid_rect = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(1));
    let status_rect = Rect::new(area.x + 1, area.bottom() - 1, area.width.saturating_sub(2), 1);

    render_terminal_grid(f, grid_rect, screen);
    render_status_bar(f, status_rect, labels, active_idx, mode, sysinfo, corner_color, bg_active);

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

#[allow(clippy::too_many_arguments)]
fn render_status_bar(f: &mut Frame, area: Rect, _labels: &[(char, String)], _active_idx: usize, mode: &Mode, sysinfo: Option<&SysInfo>, corner_color: Color, bg_active: bool) {
    let mut spans = vec![Span::styled("──", Style::default())];

    let load_bg = crate::palette::PROCESSING_BASE;
    let mem_bg = crate::palette::PROCESSING_GLOW;
    let arch_bg = crate::palette::PROCESSING_HEAT;
    let hint_bg = crate::palette::PROCESSING_PEAK;
    let white = Style::default().fg(Color::White);

    let mut fixed: usize = 2;

    let hint_text = match mode {
        Mode::Normal if bg_active => " \u{F013}  Ctrl+\\ Menu ".to_string(),
        Mode::Normal => " Ctrl+\\ Menu ".to_string(),
        Mode::Command(_) => " Esc=Close ".to_string(),
    };

    let mut sys_spans: Vec<Span<'static>> = Vec::new();

    if let Some(info) = sysinfo {
        let load_text = format!(" {} ", crate::sysinfo::load_string(info));
        let mem_text = format!(" {} ", crate::sysinfo::mem_string(info));
        let arch_text = format!(" \u{F233}  {} ", info.arch);

        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(load_bg)));
        sys_spans.push(Span::styled(load_text.clone(), white.bg(load_bg)));
        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(mem_bg).bg(load_bg)));
        sys_spans.push(Span::styled(mem_text.clone(), white.bg(mem_bg)));
        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(arch_bg).bg(mem_bg)));
        sys_spans.push(Span::styled(arch_text.clone(), white.bg(arch_bg)));
        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(hint_bg).bg(arch_bg)));
        sys_spans.push(Span::styled(hint_text.clone(), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD).bg(hint_bg)));

        fixed += 4;
        fixed += load_text.chars().count();
        fixed += mem_text.chars().count();
        fixed += arch_text.chars().count();
        fixed += hint_text.chars().count();
    } else {
        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(hint_bg)));
        sys_spans.push(Span::styled(hint_text.clone(), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD).bg(hint_bg)));
        fixed += 1;
        fixed += hint_text.chars().count();
    }

    spans.append(&mut sys_spans);

    let closing_width = 3;
    let gradient_width = (area.width as usize).saturating_sub(fixed + closing_width);

    spans.push(Span::styled(" ".repeat(gradient_width), Style::default()));
    spans.push(Span::styled("\u{E0B0}", Style::default().fg(corner_color)));
    spans.push(Span::styled("──", Style::default()));

    f.render_widget(Paragraph::new(Line::from(spans)), area);

    if gradient_width > 0 {
        let colors = blend_1d(gradient_width, &[hint_bg, corner_color]);
        let gradient_rect = GradientRect { colors, width: gradient_width as u16 };
        let gradient_area = Rect::new(area.x + fixed as u16, area.y, gradient_width as u16, 1);
        f.render_widget(&gradient_rect, gradient_area);
    }
}

fn render_menu(f: &mut Frame, area: Rect, menu: &MenuState) {
    let menu_width: u16 = 50;
    let menu_height = (menu.items.len() + 4) as u16;

    let x = area.x + (area.width.saturating_sub(menu_width)) / 2;
    let y = area.y + (area.height.saturating_sub(menu_height)) / 2;
    let menu_area = Rect::new(x, y, menu_width, menu_height);

    f.render_widget(&MenuShadow { area: menu_area }, area);

    let bg_colors = blend_2d(menu_width as usize, menu_height as usize, 10.0, &[crate::palette::BG_1, crate::palette::BG_2]);
    f.render_widget(&GradientRect { colors: bg_colors, width: menu_width }, menu_area);

    let title_line = Line::from(vec![
        Span::styled("\u{E0B2}", Style::default().fg(crate::palette::PROCESSING_PEAK)),
        Span::styled(" Tasks ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD).bg(crate::palette::PROCESSING_PEAK)),
        Span::styled("\u{E0B0}", Style::default().fg(crate::palette::PROCESSING_PEAK)),
    ]);

    let block = Block::default()
        .title(title_line)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(crate::palette::PROCESSING_PEAK));
    let inner_area = block.inner(menu_area);
    f.render_widget(block, menu_area);

    let header_rect = Rect::new(inner_area.x, inner_area.y, inner_area.width, 1);
    f.render_widget(
        &DashedTitle { text: "Applications", text_fg: crate::palette::PRIMARY, grad_start: crate::palette::PROCESSING, grad_end: crate::palette::PROCESSING_DIMMED },
        header_rect,
    );

    let items_start_y = inner_area.y + 2;
    let inner_width = inner_area.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    for (i, item) in menu.items.iter().enumerate() {
        let (label, hint) = match item {
            MenuItem::Tab { name, shortcut, .. } => (name.clone(), shortcut.to_uppercase().to_string()),
            MenuItem::Quit => ("Quit".to_string(), "Q".to_string()),
        };

        let selected = i == menu.selected;
        let item_style = if selected {
            Style::default().fg(crate::palette::BLACK).bg(crate::palette::HIGHLIGHT)
        } else {
            Style::default().fg(crate::palette::FG)
        };
        let hint_style = if selected {
            Style::default().fg(crate::palette::BG_0).bg(crate::palette::HIGHLIGHT)
        } else {
            Style::default().fg(crate::palette::PRIMARY)
        };

        let padding = inner_width.saturating_sub(label.len() + 1 + hint.len()).saturating_sub(2);
        lines.push(Line::from(vec![
            Span::styled(format!(" {label}{}", " ".repeat(padding)), item_style),
            Span::styled(hint, hint_style),
            Span::styled(" ", item_style),
        ]));
    }

    let text = Text::from(lines);
    let items_area = Rect::new(inner_area.x, items_start_y, inner_area.width, menu.items.len() as u16);
    f.render_widget(Paragraph::new(text), items_area);
}

fn cell_to_span(cell: &vt100::Cell) -> Span<'_> {
    let mut modifiers = Modifier::empty();

    for (enabled, modifier) in [
        (cell.bold(), Modifier::BOLD),
        (cell.dim(), Modifier::DIM),
        (cell.italic(), Modifier::ITALIC),
        (cell.underline(), Modifier::UNDERLINED),
        (cell.inverse(), Modifier::REVERSED),
    ] {
        if enabled {
            modifiers.insert(modifier);
        }
    }

    let style = Style::default()
        .fg(to_ratatui_color(cell.fgcolor()))
        .bg(to_ratatui_color(cell.bgcolor()))
        .add_modifier(modifiers);

    Span::styled(cell.contents().to_owned(), style)
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
