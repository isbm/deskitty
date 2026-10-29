use anyhow::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use serde::de::{self, Deserializer};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use vt100::{Cell, Color, Screen};

use crate::sounds;

/// Stable window before an observed match count is acted on.
///
/// This suppresses transient changes in the visible screen while a TUI is
/// clearing or redrawing itself.
pub(crate) const SETTLE: Duration = Duration::from_millis(250);

/// Top-level `events:` config section.
#[derive(Debug, Default, Deserialize)]
pub struct EventsConfig {
    /// Ordered text-triggered rules. Accepts one rule or a list of rules.
    #[serde(default, deserialize_with = "one_or_many")]
    pub on_text: Vec<TextEvent>,
    /// Rules that fire after their input disappears for a timeout.
    #[serde(default, deserialize_with = "one_or_many")]
    pub on_disappear: Vec<DisappearEvent>,
}

impl EventsConfig {
    /// Validate rule IDs, skip targets, and actions.
    ///
    /// Text-rule IDs must be unique and non-empty. A `skip` target must exist
    /// and must appear below the rule that references it. Actions must use
    /// known sounds, and disappearance actions cannot repeat.
    pub fn validate(&self) -> Result<()> {
        let mut ids: HashMap<&str, usize> = HashMap::new();

        for (idx, event) in self.on_text.iter().enumerate() {
            let Some(id) = event.id.as_deref() else { continue };
            if id.trim().is_empty() {
                bail!("event {} has an empty id", idx + 1);
            }
            if let Some(previous) = ids.insert(id, idx) {
                bail!("event id '{id}' is duplicated (events {} and {})", previous + 1, idx + 1);
            }
        }

        for (idx, event) in self.on_text.iter().enumerate() {
            for target in &event.skip {
                if target.trim().is_empty() {
                    bail!("event {} has an empty skip target", event_label(idx, event));
                }
                let Some(&target_idx) = ids.get(target.as_str()) else {
                    bail!("event {} skips unknown id '{target}'", event_label(idx, event));
                };
                if target_idx <= idx {
                    bail!("event {} skips '{target}', but the target must be defined below it", event_label(idx, event));
                }
            }

            if let Some(sound) = event.action.as_ref().and_then(|action| action.sound.as_deref()) {
                if sound.trim().is_empty() {
                    bail!("event {} has an empty sound name", event_label(idx, event));
                }
                if sounds::find(sound).is_none() {
                    let available = sounds::names().collect::<Vec<_>>().join(", ");
                    bail!("event {} references unknown sound '{sound}' (available: {available})", event_label(idx, event));
                }
            }
        }

        for (idx, event) in self.on_disappear.iter().enumerate() {
            if let Some(action) = &event.action {
                if action.repeat.is_some() {
                    bail!("on_disappear event {} cannot use repeat", disappear_label(idx, event));
                }
                if let Some(sound) = action.sound.as_deref() {
                    if sound.trim().is_empty() {
                        bail!("on_disappear event {} has an empty sound name", disappear_label(idx, event));
                    }
                    if sounds::find(sound).is_none() {
                        let available = sounds::names().collect::<Vec<_>>().join(", ");
                        bail!("on_disappear event {} references unknown sound '{sound}' (available: {available})", disappear_label(idx, event));
                    }
                }
            }
        }

        Ok(())
    }

    /// Whether any rule uses an embedded sound.
    pub fn uses_sounds(&self) -> bool {
        self.on_text.iter().any(|event| event.action.as_ref().is_some_and(|action| action.sound.is_some()))
            || self.on_disappear.iter().any(|event| event.action.as_ref().is_some_and(|action| action.sound.is_some()))
    }
}

fn disappear_label(idx: usize, event: &DisappearEvent) -> String {
    match event.id.as_deref() {
        Some(id) => format!("#{} ('{id}')", idx + 1),
        None => format!("#{}", idx + 1),
    }
}

/// A text trigger, optional action, and optional skip declarations.
#[derive(Debug, Clone, Deserialize)]
pub struct TextEvent {
    /// Unique rule/action identifier used by `skip` and debug output.
    #[serde(default)]
    pub id: Option<String>,
    pub input: TextInput,
    /// Keystrokes to send when this rule fires.
    #[serde(default)]
    pub action: Option<EventAction>,
    /// IDs of rules below this one whose actions should be suppressed while
    /// this rule's input is visible.
    #[serde(default, deserialize_with = "one_or_many")]
    pub skip: Vec<String>,
}

/// A rule that fires after its input disappears for a timeout.
#[derive(Debug, Clone, Deserialize)]
pub struct DisappearEvent {
    /// Optional identifier used in debug output.
    #[serde(default)]
    pub id: Option<String>,
    pub input: TextInput,
    /// How long the input must remain absent before the action fires.
    #[serde(deserialize_with = "deserialize_timeout")]
    pub timeout: Duration,
    /// Action performed when the timeout expires.
    #[serde(default)]
    pub action: Option<EventAction>,
}

impl DisappearEvent {
    /// Count visible occurrences that satisfy the text and background filters.
    pub fn match_count(&self, screen: &Screen) -> usize {
        self.input.match_count(screen)
    }

    /// Sample every visible content match for debug scanning.
    pub(crate) fn scan_samples(&self, screen: &Screen) -> Vec<ScanSample> {
        self.input.scan_samples(screen)
    }
}

fn deserialize_timeout<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = f64::deserialize(deserializer)?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(de::Error::custom("timeout must be a positive number of seconds"));
    }
    if seconds > 86_400.0 {
        return Err(de::Error::custom("timeout must be at most 86400 seconds"));
    }
    Ok(Duration::from_secs_f64(seconds))
}

/// Visible-text pattern to match.
#[derive(Debug, Clone, Deserialize)]
pub struct TextInput {
    /// Literal, case-sensitive text to find on the visible screen.
    pub content: String,
    /// Optional background-color allowlist.
    ///
    /// When empty, any background matches. When non-empty, every non-space
    /// character in the match must have one of these backgrounds.
    #[serde(default, deserialize_with = "color_list")]
    pub background: Vec<ColorSpec>,
}

/// Action performed when a rule fires.
#[derive(Debug, Clone, Deserialize)]
pub struct EventAction {
    /// Keystrokes to send in order.
    #[serde(default, deserialize_with = "one_or_many")]
    pub keystroke: Vec<Keystroke>,
    /// Delay between consecutive keystrokes, in seconds.
    #[serde(default, deserialize_with = "deserialize_delay")]
    pub delay: Duration,
    /// Optional embedded sound to play when the action fires.
    #[serde(default)]
    pub sound: Option<String>,
    /// Repeat the action every N seconds while the match remains visible.
    #[serde(default, deserialize_with = "deserialize_repeat")]
    pub repeat: Option<Duration>,
}

fn deserialize_repeat<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = f64::deserialize(deserializer)?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(de::Error::custom("repeat must be a positive number of seconds"));
    }
    if seconds > 86_400.0 {
        return Err(de::Error::custom("repeat must be at most 86400 seconds"));
    }
    Ok(Some(Duration::from_secs_f64(seconds)))
}

fn deserialize_delay<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = f64::deserialize(deserializer)?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(de::Error::custom("delay must be a non-negative number of seconds"));
    }
    if seconds > 86_400.0 {
        return Err(de::Error::custom("delay must be at most 86400 seconds"));
    }
    Ok(Duration::from_secs_f64(seconds))
}

/// Result of evaluating the ordered rule list for one tick.
#[derive(Debug, Default)]
pub struct EventRun {
    pub outcomes: Vec<EventOutcome>,
}

/// What happened to a rule during one evaluation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    /// The rule's keystroke action would fire.
    Fired { event: usize },
    /// The rule matched and would have fired, but an earlier rule skipped it.
    Skipped { event: usize, by: usize },
}

/// Evaluate rules top-to-bottom, updating gates and applying skip guards.
///
/// Skip declarations are based on whether the guard's input currently matches,
/// not on whether the guard's own action fires. A skipped rule still consumes
/// its gate so it cannot fire later when the guard disappears.
pub fn evaluate_events(events: &[TextEvent], counts: &[usize], gates: &mut [MatchGate], now: Instant) -> EventRun {
    let mut skip: HashMap<&str, usize> = HashMap::new();
    let mut run = EventRun::default();

    for (idx, event) in events.iter().enumerate() {
        let count = counts.get(idx).copied().unwrap_or(0);
        if count > 0 {
            for target in &event.skip {
                skip.entry(target.as_str()).or_insert(idx);
            }
        }

        let repeat = event.action.as_ref().and_then(repeat_interval);
        let fired = match repeat {
            Some(interval) => gates[idx].update_repeating(count, now, interval),
            None => gates[idx].update(count, now),
        };
        if !fired {
            continue;
        }

        let has_action = event.action.as_ref().is_some_and(|action| !action.keystroke.is_empty() || action.sound.is_some());
        if let Some(id) = event.id.as_deref()
            && let Some(&by) = skip.get(id)
        {
            if has_action {
                run.outcomes.push(EventOutcome::Skipped { event: idx, by });
            }
            continue;
        }

        if has_action {
            run.outcomes.push(EventOutcome::Fired { event: idx });
        }
    }

    run
}

/// Effective repetition interval, including the time needed to send the
/// action's own keystroke block.
fn repeat_interval(action: &EventAction) -> Option<Duration> {
    action.repeat.map(|repeat| {
        let gaps = action.keystroke.len().saturating_sub(1);
        let block = action.delay.saturating_mul(u32::try_from(gaps).unwrap_or(u32::MAX));
        repeat.saturating_add(block)
    })
}

fn event_label(idx: usize, event: &TextEvent) -> String {
    match event.id.as_deref() {
        Some(id) => format!("#{} ('{id}')", idx + 1),
        None => format!("#{}", idx + 1),
    }
}

impl TextEvent {
    /// Count visible occurrences that satisfy the text and background filters.
    ///
    /// Matching works across wrapped rows within the visible screen. Explicit
    /// newlines in `content` are not supported.
    pub fn match_count(&self, screen: &Screen) -> usize {
        self.input.match_count(screen)
    }

    /// Sample every visible content match for debug scanning.
    pub(crate) fn scan_samples(&self, screen: &Screen) -> Vec<ScanSample> {
        self.input.scan_samples(screen)
    }
}

impl TextInput {
    /// Count visible occurrences that satisfy the text and background filters.
    pub fn match_count(&self, screen: &Screen) -> usize {
        self.match_windows(screen).iter().filter(|window| self.background_matches(screen, window)).count()
    }

    /// Sample every visible content match for debug scanning.
    ///
    /// Unlike [`match_count`](Self::match_count), this ignores the configured
    /// background so a wrong selector still produces useful diagnostic data.
    pub(crate) fn scan_samples(&self, screen: &Screen) -> Vec<ScanSample> {
        self.match_windows(screen).into_iter().filter_map(|window| self.scan_sample(screen, &window)).collect()
    }

    fn match_windows(&self, screen: &Screen) -> Vec<Vec<ScreenChar>> {
        let needle: Vec<char> = self.content.chars().collect();
        if needle.is_empty() {
            return Vec::new();
        }

        let (rows, cols) = screen.size();
        let mut line = Vec::new();
        let mut windows = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else { continue };
                if cell.is_wide_continuation() {
                    continue;
                }

                let contents = cell.contents();
                if contents.is_empty() {
                    line.push(ScreenChar { ch: ' ', row, col });
                } else {
                    for ch in contents.chars() {
                        line.push(ScreenChar { ch, row, col });
                    }
                }
            }

            if !screen.row_wrapped(row) || row + 1 == rows {
                collect_windows(&line, &needle, &mut windows);
                line.clear();
            }
        }
        windows
    }

    fn scan_sample(&self, screen: &Screen, window: &[ScreenChar]) -> Option<ScanSample> {
        let cell = window
            .iter()
            .find(|cell| !cell.ch.is_whitespace())
            .and_then(|cell| screen.cell(cell.row, cell.col))
            .or_else(|| window.first().and_then(|cell| screen.cell(cell.row, cell.col)))?;

        let mut backgrounds = Vec::new();
        for span in window.iter().filter(|cell| !cell.ch.is_whitespace()) {
            let Some(span_cell) = screen.cell(span.row, span.col) else { continue };
            let color = ColorSpec::from_terminal(effective_bg(span_cell));
            if !backgrounds.contains(&color) {
                backgrounds.push(color);
            }
        }

        let background_ok = if self.background.is_empty() { None } else { Some(self.background_matches(screen, window)) };
        Some(ScanSample {
            fg: cell.fgcolor(),
            bg: cell.bgcolor(),
            backgrounds,
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
            background_ok,
        })
    }

    fn background_matches(&self, screen: &Screen, window: &[ScreenChar]) -> bool {
        if self.background.is_empty() {
            return true;
        }

        window
            .iter()
            .filter(|cell| !cell.ch.is_whitespace())
            .all(|cell| screen.cell(cell.row, cell.col).is_some_and(|cell| self.background.iter().any(|spec| spec.matches(effective_bg(cell)))))
    }
}

fn collect_windows(line: &[ScreenChar], needle: &[char], windows: &mut Vec<Vec<ScreenChar>>) {
    let mut pos = 0;
    while pos + needle.len() <= line.len() {
        let window = &line[pos..pos + needle.len()];
        if window.iter().map(|cell| cell.ch).eq(needle.iter().copied()) {
            windows.push(window.to_vec());
            pos += needle.len();
        } else {
            pos += 1;
        }
    }
}

/// Attributes of one matching screen span, used by debug scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSample {
    /// Raw foreground color of the first non-space cell.
    pub fg: Color,
    /// Raw background color of the first non-space cell.
    pub bg: Color,
    /// Distinct effective backgrounds across the matched span.
    pub backgrounds: Vec<ColorSpec>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    /// Whether the event's configured background matches this span.
    /// `None` when the event has no background condition.
    pub background_ok: Option<bool>,
}

#[derive(Clone, Copy)]
struct ScreenChar {
    ch: char,
    row: u16,
    col: u16,
}

/// A color condition from an event's `background` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpec {
    /// The terminal's default background.
    Default,
    /// A named ANSI color family.
    Named(NamedColor),
    /// A specific ANSI palette index.
    Ansi(u8),
    /// A specific RGB color.
    Rgb(u8, u8, u8),
}

impl ColorSpec {
    /// Parse a color name, YAML tag name, palette index, or `#RRGGBB` value.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.strip_prefix('!').unwrap_or(raw).trim();
        if raw.eq_ignore_ascii_case("default") {
            return Some(Self::Default);
        }
        if let Some(hex) = raw.strip_prefix('#') {
            return parse_hex(hex).map(|(r, g, b)| Self::Rgb(r, g, b));
        }
        if let Ok(index) = raw.parse::<u8>() {
            return Some(Self::Ansi(index));
        }
        NamedColor::parse(raw).map(Self::Named)
    }

    /// Build a color condition from a YAML value.
    fn from_value(value: serde_yaml::Value) -> Result<Self, String> {
        let raw = match value {
            serde_yaml::Value::String(value) => value,
            serde_yaml::Value::Tagged(tagged) => tagged.tag.to_string(),
            serde_yaml::Value::Number(value) => value.to_string(),
            other => return Err(format!("expected a color, found {other:?}")),
        };
        Self::parse(&raw).ok_or_else(|| format!("unknown color: {raw}"))
    }

    /// Return whether `color` satisfies this condition.
    pub fn matches(self, color: Color) -> bool {
        match self {
            Self::Default => matches!(color, Color::Default),
            Self::Ansi(index) => matches!(color, Color::Idx(actual) if actual == index),
            Self::Rgb(r, g, b) => normalized_rgb(color).is_some_and(|actual| actual == (r, g, b)),
            Self::Named(name) => normalized_rgb(color).is_some_and(|actual| name.matches_rgb(actual)),
        }
    }

    /// Convert a terminal color to its canonical config form.
    ///
    /// Indexed colors are resolved through the xterm palette so the resulting
    /// `#rrggbb` value also matches truecolor output.
    pub fn from_terminal(color: Color) -> Self {
        match color {
            Color::Default => Self::Default,
            Color::Idx(index) => {
                let (r, g, b) = palette_rgb(index);
                Self::Rgb(r, g, b)
            }
            Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
        }
    }

    /// Render this color as a YAML scalar for an event's color list.
    pub fn to_yaml(self) -> String {
        match self {
            Self::Default => "\"!default\"".to_string(),
            Self::Ansi(index) => index.to_string(),
            Self::Rgb(r, g, b) => format!("\"#{r:02x}{g:02x}{b:02x}\""),
            Self::Named(name) => name.as_str().to_string(),
        }
    }

    /// Render this color for compact debug output.
    pub fn display(self) -> String {
        match self {
            Self::Default => "def".to_string(),
            Self::Ansi(index) => format!("i{index}"),
            Self::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            Self::Named(name) => name.as_str().to_string(),
        }
    }
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

impl<'de> Deserialize<'de> for ColorSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_value(serde_yaml::Value::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn color_list<'de, D>(deserializer: D) -> Result<Vec<ColorSpec>, D::Error>
where
    D: Deserializer<'de>,
{
    one_or_many_value(serde_yaml::Value::deserialize(deserializer)?).map_err(de::Error::custom)
}

/// Named color families used by the event matcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
}

impl NamedColor {
    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.to_ascii_lowercase();
        let raw = raw.strip_prefix("bright_").unwrap_or(&raw);
        match raw {
            "black" => Some(Self::Black),
            "red" => Some(Self::Red),
            "green" => Some(Self::Green),
            "yellow" => Some(Self::Yellow),
            "blue" => Some(Self::Blue),
            "magenta" | "purple" => Some(Self::Magenta),
            "cyan" => Some(Self::Cyan),
            "white" => Some(Self::White),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Cyan => "cyan",
            Self::White => "white",
            Self::Gray => "gray",
        }
    }

    fn matches_rgb(self, (r, g, b): (u8, u8, u8)) -> bool {
        match self {
            Self::Black => r.max(g).max(b) <= 64,
            Self::Red => r > g && r > b,
            Self::Green => g > r && g > b,
            Self::Yellow => r == g && r > b,
            Self::Blue => b > r && b > g,
            Self::Magenta => r == b && r > g,
            Self::Cyan => g == b && g > r,
            Self::White => r.min(g).min(b) >= 192,
            Self::Gray => r == g && g == b && r > 64 && r < 192,
        }
    }
}

fn effective_bg(cell: &Cell) -> Color {
    if cell.inverse() { cell.fgcolor() } else { cell.bgcolor() }
}

/// Normalize a terminal color to RGB using the standard xterm 256 palette.
///
/// This lets a configured `#005f00` match either a truecolor cell or the
/// equivalent 256-color palette entry, and lets named colors match truecolor
/// theme variants such as dark-green selection backgrounds.
fn normalized_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Default => None,
        Color::Idx(index) => Some(palette_rgb(index)),
        Color::Rgb(r, g, b) => Some((r, g, b)),
    }
}

fn palette_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            let index = index - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;
            (cube_axis(r), cube_axis(g), cube_axis(b))
        }
        232..=255 => {
            let value = 8 + (index - 232) * 10;
            (value, value, value)
        }
    }
}

fn cube_axis(value: u8) -> u8 {
    if value == 0 { 0 } else { 55 + value * 40 }
}

/// A keystroke action from the config.
#[derive(Debug, Clone, Copy)]
pub struct Keystroke(pub KeyEvent);

impl Keystroke {
    /// Convert the keystroke to the bytes written to the PTY.
    ///
    /// `app_cursor` selects the application-cursor escape sequences for arrow
    /// and navigation keys.
    pub fn to_bytes(self, app_cursor: bool) -> Option<Vec<u8>> {
        if self.0.code == KeyCode::Tab && self.0.modifiers.contains(KeyModifiers::SHIFT) {
            return Some(b"\x1b[Z".to_vec());
        }
        crate::compositor::key_to_bytes(&self.0, app_cursor)
    }

    /// Human-readable label used by debug output.
    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.0.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("CTRL".to_string());
        }
        if self.0.modifiers.contains(KeyModifiers::ALT) {
            parts.push("ALT".to_string());
        }
        if self.0.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("SHIFT".to_string());
        }

        let key = match self.0.code {
            KeyCode::Enter => "ENTER".to_string(),
            KeyCode::Esc => "ESC".to_string(),
            KeyCode::Tab => "TAB".to_string(),
            KeyCode::Backspace => "BACKSPACE".to_string(),
            KeyCode::Delete => "DELETE".to_string(),
            KeyCode::Insert => "INSERT".to_string(),
            KeyCode::Up => "UP".to_string(),
            KeyCode::Down => "DOWN".to_string(),
            KeyCode::Left => "LEFT".to_string(),
            KeyCode::Right => "RIGHT".to_string(),
            KeyCode::Home => "HOME".to_string(),
            KeyCode::End => "END".to_string(),
            KeyCode::PageUp => "PAGEUP".to_string(),
            KeyCode::PageDown => "PAGEDOWN".to_string(),
            KeyCode::Char(' ') => "SPACE".to_string(),
            KeyCode::Char(ch) => ch.to_string(),
            KeyCode::F(number) => format!("F{number}"),
            other => format!("{other:?}"),
        };
        parts.push(key);
        parts.join("+")
    }
}

impl<'de> Deserialize<'de> for Keystroke {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        parse_keystroke(&raw).map(Self).ok_or_else(|| de::Error::custom(format!("unsupported keystroke: {raw}")))
    }
}

fn parse_keystroke(raw: &str) -> Option<KeyEvent> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let parts: Vec<&str> = if raw == "+" { vec!["+"] } else { raw.split('+').map(str::trim).collect() };
    let key = *parts.last()?;
    let mut modifiers = KeyModifiers::NONE;
    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers.insert(KeyModifiers::CONTROL),
            "ALT" | "OPTION" => modifiers.insert(KeyModifiers::ALT),
            "SHIFT" => modifiers.insert(KeyModifiers::SHIFT),
            _ => return None,
        }
    }

    let upper = key.to_ascii_uppercase();
    let code = match upper.as_str() {
        "ENTER" | "RETURN" => KeyCode::Enter,
        "ESC" | "ESCAPE" => KeyCode::Esc,
        "TAB" => KeyCode::Tab,
        "BACKSPACE" => KeyCode::Backspace,
        "DELETE" | "DEL" => KeyCode::Delete,
        "INSERT" | "INS" => KeyCode::Insert,
        "UP" | "ARROWUP" => KeyCode::Up,
        "DOWN" | "ARROWDOWN" => KeyCode::Down,
        "LEFT" | "ARROWLEFT" => KeyCode::Left,
        "RIGHT" | "ARROWRIGHT" => KeyCode::Right,
        "HOME" => KeyCode::Home,
        "END" => KeyCode::End,
        "PAGEUP" | "PGUP" => KeyCode::PageUp,
        "PAGEDOWN" | "PGDN" => KeyCode::PageDown,
        "SPACE" => KeyCode::Char(' '),
        name if name.starts_with('F') => {
            let number = name[1..].parse::<u8>().ok()?;
            if !(1..=12).contains(&number) {
                return None;
            }
            KeyCode::F(number)
        }
        _ => {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => KeyCode::Char(ch),
                _ => return None,
            }
        }
    };

    let event = KeyEvent::new(code, modifiers);
    crate::compositor::key_to_bytes(&event, false)?;
    Some(event)
}

/// Indices of `on_disappear` rules that fired this tick.
#[derive(Debug, Default)]
pub struct DisappearRun {
    pub fired: Vec<usize>,
}

/// Per-tab, per-rule disappearance state.
#[derive(Debug, Default, Clone)]
pub struct DisappearGate {
    state: DisappearState,
}

#[derive(Debug, Default, Clone, Copy)]
enum DisappearState {
    #[default]
    Waiting,
    Visible,
    Missing {
        since: Instant,
    },
    Fired,
}

impl DisappearGate {
    /// Feed the current match count.
    ///
    /// Returns `true` once when a previously visible input has been absent for
    /// at least `timeout`.
    pub fn update(&mut self, count: usize, now: Instant, timeout: Duration) -> bool {
        match self.state {
            DisappearState::Waiting => {
                if count > 0 {
                    self.state = DisappearState::Visible;
                }
                false
            }
            DisappearState::Visible => {
                if count == 0 {
                    self.state = DisappearState::Missing { since: now };
                }
                false
            }
            DisappearState::Missing { since } => {
                if count > 0 {
                    self.state = DisappearState::Visible;
                    false
                } else if now.saturating_duration_since(since) >= timeout {
                    self.state = DisappearState::Fired;
                    true
                } else {
                    false
                }
            }
            DisappearState::Fired => {
                if count > 0 {
                    self.state = DisappearState::Visible;
                }
                false
            }
        }
    }

    /// Reset the gate to its initial state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Per-tab, per-event occurrence gate.
///
/// The gate fires at most once when the visible match count rises above the
/// number already consumed. Repeated redraws at the same count do nothing.
/// Count changes must remain stable for [`SETTLE`] before they are acted on.
#[derive(Debug, Default, Clone)]
pub struct MatchGate {
    consumed: usize,
    pending: Option<(usize, Instant)>,
    repeat_at: Option<Instant>,
}

impl MatchGate {
    /// Feed the current match count.
    ///
    /// Returns `true` exactly once for each stable increase.
    pub fn update(&mut self, count: usize, now: Instant) -> bool {
        match self.pending {
            Some((pending, since)) if pending == count => {
                if now.saturating_duration_since(since) < SETTLE {
                    return false;
                }
                let fire = count > self.consumed;
                self.consumed = count;
                fire
            }
            _ => {
                self.pending = Some((count, now));
                false
            }
        }
    }

    /// Feed the current match count in repeating mode.
    ///
    /// The first fire happens after the normal settle window. While the count
    /// remains non-zero, the gate fires again every `interval`. A stable zero
    /// count resets the cadence.
    pub fn update_repeating(&mut self, count: usize, now: Instant, interval: Duration) -> bool {
        match self.pending {
            Some((pending, since)) if pending == count => {
                if now.saturating_duration_since(since) < SETTLE {
                    return false;
                }
                if count == 0 {
                    self.consumed = 0;
                    self.repeat_at = None;
                    return false;
                }
                let due = self.repeat_at.is_none_or(|at| now >= at);
                if due {
                    self.consumed = count;
                    self.repeat_at = Some(now + interval);
                    true
                } else {
                    self.consumed = count;
                    false
                }
            }
            _ => {
                self.pending = Some((count, now));
                false
            }
        }
    }

    /// Reset the gate to its initial state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    one_or_many_value(serde_yaml::Value::deserialize(deserializer)?).map_err(de::Error::custom)
}

fn one_or_many_value<T>(value: serde_yaml::Value) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    match value {
        serde_yaml::Value::Null => Ok(Vec::new()),
        serde_yaml::Value::Sequence(sequence) => sequence.into_iter().map(|value| T::deserialize(value).map_err(|err| err.to_string())).collect(),
        other => Ok(vec![T::deserialize(other).map_err(|err| err.to_string())?]),
    }
}
