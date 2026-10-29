use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_glamour::color::{blend_1d, blend_2d, lerp_color};
use ratatui_glamour::rule::dashed_title;
use ratatui_glamour::surface::{centered_rect, gradient_rounded_panel_lines, render_gradient_rounded_panel};

use crate::scan::ScanState;
use crate::sysinfo::SysInfo;

/// An entry in the command-mode popup menu.
pub enum MenuItem {
    /// Switch to a tab.
    Tab { idx: usize, shortcut: char, name: String },
    /// Pause or resume automatic event keystrokes.
    ToggleActions { stopped: bool, enabled: bool },
    /// Hide or show the colored frame and status bar.
    ToggleDesktop { visible: bool },
    /// Quit deskitty entirely.
    Quit,
}

impl MenuItem {
    /// Whether the item can be focused and activated.
    pub fn selectable(&self) -> bool {
        !matches!(self, Self::ToggleActions { enabled: false, .. })
    }
}

/// State used to build the command menu.
#[derive(Debug, Clone, Copy)]
pub struct MenuOptions {
    /// Whether automatic event keystrokes are paused.
    pub actions_stopped: bool,
    /// Whether the colored frame and status bar are visible.
    pub show_desktop: bool,
    /// Whether the actions toggle can be used (false in debug mode).
    pub actions_enabled: bool,
}

/// The command-mode popup menu state.
pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected: usize,
}

impl MenuState {
    /// Build a menu from tab labels, followed by the available toggles and a
    /// `Quit` entry.
    pub fn new(labels: &[(char, String)], options: MenuOptions) -> Self {
        let mut items: Vec<MenuItem> =
            labels.iter().enumerate().map(|(idx, (shortcut, name))| MenuItem::Tab { idx, shortcut: *shortcut, name: name.clone() }).collect();
        items.push(MenuItem::ToggleActions { stopped: options.actions_stopped, enabled: options.actions_enabled });
        items.push(MenuItem::ToggleDesktop { visible: options.show_desktop });
        items.push(MenuItem::Quit);
        let selected = items.iter().position(MenuItem::selectable).unwrap_or(0);
        Self { items, selected }
    }

    /// Move selection to the next selectable item (wrapping).
    pub fn next(&mut self) {
        for offset in 1..=self.items.len() {
            let idx = (self.selected + offset) % self.items.len();
            if self.items[idx].selectable() {
                self.selected = idx;
                return;
            }
        }
    }

    /// Move selection to the previous selectable item (wrapping).
    pub fn prev(&mut self) {
        for offset in 1..=self.items.len() {
            let idx = (self.selected + self.items.len() - offset) % self.items.len();
            if self.items[idx].selectable() {
                self.selected = idx;
                return;
            }
        }
    }

    /// Find the index of the item matching the given shortcut character.
    ///
    /// Returns `None` if no item matches or the matching item is disabled.
    pub fn find_by_shortcut(&self, c: char) -> Option<usize> {
        self.items.iter().position(|item| match item {
            MenuItem::Tab { shortcut, .. } => *shortcut == c,
            MenuItem::ToggleActions { enabled, .. } => *enabled && c == 'a',
            MenuItem::ToggleDesktop { .. } => c == 'd',
            MenuItem::Quit => c == 'q',
        })
    }
}

/// A modal notice window.
pub struct Notice {
    /// Window headline.
    pub title: String,
    /// Body lines, centered one per row.
    pub lines: Vec<String>,
}

impl Notice {
    /// Create a modal notice with a title and body lines.
    pub fn new(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self { title: title.into(), lines }
    }
}

/// Current interaction mode.
pub enum Mode {
    /// Keys are forwarded to the active tab's PTY.
    Normal,
    /// A popup menu is displayed; keys navigate the menu.
    Command(MenuState),
    /// A modal notice is displayed; any dismiss key closes it.
    Notice(Notice),
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
#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame, screen: &vt100::Screen, labels: &[(char, String)], active_idx: usize, mode: &Mode, sysinfo: Option<&SysInfo>, bg_active: bool,
    show_desktop: bool, scan: &ScanState,
) {
    let area = f.area();

    if show_desktop {
        let border_stops =
            vec![Color::Rgb(255, 95, 135), Color::Rgb(135, 95, 255), Color::Rgb(0, 255, 255), Color::Rgb(135, 95, 255), Color::Rgb(255, 95, 135)];
        let corner_color = bottom_right_corner_color(area, &border_stops);
        f.render_widget(&GradientPanel { stops: border_stops }, area);

        let inner = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(2));
        let grid_rect = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(1));
        let status_rect = Rect::new(area.x + 1, area.bottom() - 1, area.width.saturating_sub(2), 1);

        render_terminal_grid(f, grid_rect, screen);
        render_status_bar(f, status_rect, labels, active_idx, mode, sysinfo, corner_color, bg_active);
    } else {
        render_terminal_grid(f, area, screen);
    }

    if scan.active() {
        render_scan_overlay(f, area, scan);
    }

    if let Mode::Command(menu) = mode {
        render_menu(f, area, menu);
    }

    if let Mode::Notice(notice) = mode {
        render_notice(f, area, notice);
    }
}

fn render_notice(f: &mut Frame, area: Rect, notice: &Notice) {
    const BORDER: [Color; 5] = [Color::Indexed(205), Color::Indexed(99), Color::Indexed(51), Color::Indexed(99), Color::Indexed(205)];
    const TEXT: [Color; 3] = [Color::Rgb(255, 240, 100), Color::Rgb(255, 170, 50), Color::Rgb(255, 50, 50)];

    let width = 62.min(area.width.saturating_sub(4));
    let height = 10.min(area.height.saturating_sub(2));
    if width < 20 || height < 6 {
        return;
    }

    let dialog = centered_rect(area, width, height);
    f.render_widget(&MenuShadow { area: dialog }, area);

    let inner = render_gradient_rounded_panel(f.buffer_mut(), dialog, Style::default().bg(Color::Indexed(234)), &BORDER);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let background = blend_2d(inner.width as usize, inner.height as usize, 10.0, &[Color::Rgb(92, 62, 146), Color::Rgb(30, 20, 52)]);
    f.render_widget(&GradientRect { colors: background, width: inner.width }, inner);

    put_centered_gradient_text(f.buffer_mut(), inner, inner.y + 1, &notice.title, &TEXT);

    for (idx, line) in notice.lines.iter().enumerate() {
        let y = inner.y + 3 + idx as u16;
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        put_centered_gradient_text(f.buffer_mut(), inner, y, line, &TEXT);
    }

    let button_width = 13.min(inner.width);
    let button = Rect::new(inner.x + inner.width.saturating_sub(button_width) / 2, inner.bottom().saturating_sub(1), button_width, 1);
    draw_notice_button(f.buffer_mut(), button, "Fine  :-(");
}

fn put_centered_gradient_text(buf: &mut Buffer, area: Rect, y: u16, text: &str, stops: &[Color]) {
    if area.width == 0 || y >= area.bottom() {
        return;
    }
    let chars: Vec<char> = text.chars().take(area.width as usize).collect();
    if chars.is_empty() {
        return;
    }
    let x = area.x + area.width.saturating_sub(chars.len() as u16) / 2;
    let colors = blend_1d(chars.len(), stops);
    for (idx, (ch, color)) in chars.into_iter().zip(colors).enumerate() {
        let cx = x + idx as u16;
        if cx >= area.right() {
            break;
        }
        if let Some(cell) = buf.cell_mut((cx, y)) {
            cell.set_char(ch);
            cell.set_fg(color);
        }
    }
}

fn draw_notice_button(buf: &mut Buffer, area: Rect, label: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = Color::Indexed(205);
    let fg = Color::Indexed(231);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_bg(bg);
                cell.set_fg(fg);
            }
        }
    }

    let chars: Vec<char> = label.chars().take(area.width as usize).collect();
    let x = area.x + area.width.saturating_sub(chars.len() as u16) / 2;
    for (idx, ch) in chars.into_iter().enumerate() {
        if let Some(cell) = buf.cell_mut((x + idx as u16, area.y)) {
            cell.set_char(ch);
            cell.set_fg(fg);
            cell.set_bg(bg);
            cell.set_style(Style::default().add_modifier(Modifier::UNDERLINED));
        }
    }
}

fn render_scan_overlay(f: &mut Frame, area: Rect, scan: &ScanState) {
    let width = 50.min(area.width.saturating_sub(5));
    let height = 5.min(area.height.saturating_sub(2));
    if width < 10 || height < 2 {
        return;
    }

    let x = area.x + area.width.saturating_sub(width + 4);
    let y = area.y + 1;
    let rect = Rect::new(x, y, width, height);
    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::TOP)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(crate::palette::PRIMARY))
        .title(Span::styled(format!(" {} ", scan.title()), Style::default().fg(crate::palette::PRIMARY).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(crate::palette::BG_1));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let lines: Vec<_> = scan.lines().collect();
    let start = lines.len().saturating_sub(inner.height as usize);
    let lines: Vec<Line> = lines[start..]
        .iter()
        .map(|line| {
            let mut spans = Vec::new();
            if let Some((fg, bg)) = line.swatch {
                spans.push(Span::styled("  ", Style::default().fg(to_ratatui_color(fg)).bg(to_ratatui_color(bg))));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(line.text.clone(), Style::default().fg(crate::palette::FG)));
            Line::from(spans)
        })
        .collect();

    let text = if lines.is_empty() {
        Text::from(Line::from(Span::styled("waiting...", Style::default().fg(crate::palette::GRAY_1))))
    } else {
        Text::from(lines)
    };
    f.render_widget(Paragraph::new(text).style(Style::default().bg(crate::palette::BG_1)), inner);
}

fn render_terminal_grid(f: &mut Frame, area: Rect, screen: &vt100::Screen) {
    let (rows, cols) = screen.size();
    let max_rows = area.height.min(rows);
    let max_cols = area.width.min(cols);
    let buf = f.buffer_mut();

    for row in 0..max_rows {
        let mut col: u16 = 0;
        while col < max_cols {
            let x = area.x + col;
            let y = area.y + row;

            if let Some(cell) = screen.cell(row, col) {
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }

                let mut style = Style::default()
                    .fg(to_ratatui_color(cell.fgcolor()))
                    .bg(to_ratatui_color(cell.bgcolor()));

                if cell.bold()      { style = style.add_modifier(Modifier::BOLD); }
                if cell.dim()       { style = style.add_modifier(Modifier::DIM); }
                if cell.italic()    { style = style.add_modifier(Modifier::ITALIC); }
                if cell.underline() { style = style.add_modifier(Modifier::UNDERLINED); }
                if cell.inverse()   { style = style.add_modifier(Modifier::REVERSED); }

                let ch = cell.contents();
                let display = if ch.is_empty() { " " } else { ch };

                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_symbol(display);
                    c.set_style(style);
                }

                if cell.is_wide() {
                    if col + 1 < max_cols
                        && let Some(c) = buf.cell_mut((x + 1, y))
                    {
                        c.set_symbol(" ");
                        c.set_style(style);
                    }
                    col += 2;
                } else {
                    col += 1;
                }
            } else {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_symbol(" ");
                    c.set_style(Style::default());
                }
                col += 1;
            }
        }
    }

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
        Mode::Normal => " Ctrl+\\ Menu ".to_string(),
        Mode::Command(_) => " ESC Close ".to_string(),
        Mode::Notice(_) => " OK ".to_string(),
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
        sys_spans.push(Span::styled(hint_text.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD).bg(hint_bg)));

        fixed += 4;
        fixed += load_text.chars().count();
        fixed += mem_text.chars().count();
        fixed += arch_text.chars().count();
        fixed += hint_text.chars().count();
    } else {
        sys_spans.push(Span::styled("\u{E0B2}", Style::default().fg(hint_bg)));
        sys_spans.push(Span::styled(hint_text.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD).bg(hint_bg)));
        fixed += 1;
        fixed += hint_text.chars().count();
    }

    spans.append(&mut sys_spans);

    let closing_width = if bg_active { 5 } else { 3 };
    let gradient_width = (area.width as usize).saturating_sub(fixed + closing_width);

    spans.push(Span::styled(" ".repeat(gradient_width), Style::default()));
    if bg_active {
        spans.push(Span::styled("\u{2699} ", Style::default().fg(crate::palette::ERROR).bg(corner_color)));
    }
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
            MenuItem::ToggleActions { stopped, enabled } => {
                if *enabled {
                    let label = if *stopped { "Resume actions" } else { "Stop actions" };
                    (label.to_string(), "A".to_string())
                } else {
                    ("Actions paused".to_string(), "debug".to_string())
                }
            }
            MenuItem::ToggleDesktop { visible } => {
                let label = if *visible { "Hide desktop" } else { "Show desktop" };
                (label.to_string(), "D".to_string())
            }
            MenuItem::Quit => ("Quit".to_string(), "Q".to_string()),
        };

        let disabled = !item.selectable();
        let selected = !disabled && i == menu.selected;
        let item_style = if disabled {
            Style::default().fg(crate::palette::GRAY_1)
        } else if selected {
            Style::default().fg(crate::palette::BLACK).bg(crate::palette::HIGHLIGHT)
        } else {
            Style::default().fg(crate::palette::FG)
        };
        let hint_style = if disabled {
            Style::default().fg(crate::palette::FAINT)
        } else if selected {
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
pub fn key_to_bytes(key: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
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
        KeyCode::Up => if app_cursor { Some(b"\x1bOA".to_vec()) } else { Some(b"\x1b[A".to_vec()) },
        KeyCode::Down => if app_cursor { Some(b"\x1bOB".to_vec()) } else { Some(b"\x1b[B".to_vec()) },
        KeyCode::Right => if app_cursor { Some(b"\x1bOC".to_vec()) } else { Some(b"\x1b[C".to_vec()) },
        KeyCode::Left => if app_cursor { Some(b"\x1bOD".to_vec()) } else { Some(b"\x1b[D".to_vec()) },
        KeyCode::Home => if app_cursor { Some(b"\x1bOH".to_vec()) } else { Some(b"\x1b[H".to_vec()) },
        KeyCode::End => if app_cursor { Some(b"\x1bOF".to_vec()) } else { Some(b"\x1b[F".to_vec()) },
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
