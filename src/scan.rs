use crate::events::{ColorSpec, EventAction, EventOutcome, EventRun, ScanSample, TextEvent};
use std::collections::VecDeque;
use vt100::Screen;

const MAX_LINES: usize = 128;

/// One rendered line in the debug overlay log.
#[derive(Debug, Clone)]
pub struct ScanLine {
    pub text: String,
    /// Optional foreground/background swatch for sampled attribute lines.
    pub swatch: Option<(vt100::Color, vt100::Color)>,
}

/// Debug scan state: a rolling log of observed match attributes and would-fire
/// or skipped events.
#[derive(Debug, Default)]
pub struct ScanState {
    lines: VecDeque<ScanLine>,
    last: Vec<Vec<String>>,
    title: String,
    live: usize,
    fires: usize,
    skips: usize,
    active: bool,
}

impl ScanState {
    /// Update the scan log from the current screen and the evaluated rule
    /// outcomes for this tick.
    ///
    /// Returns `true` when the overlay needs to be redrawn.
    pub fn update(&mut self, events: &[TextEvent], screen: &Screen, run: &EventRun) -> bool {
        if events.is_empty() {
            let changed = self.active;
            self.active = false;
            self.last.clear();
            self.title.clear();
            self.live = 0;
            return changed;
        }

        self.active = true;
        self.last.resize_with(events.len(), Vec::new);

        let multiple = events.len() > 1;
        let mut changed = false;
        let mut live = 0;

        for (idx, event) in events.iter().enumerate() {
            let samples = event.scan_samples(screen);
            live += samples.len();

            let mut current = Vec::new();
            for sample in samples {
                let background = background_line(idx, &sample, multiple);
                let attributes = attribute_line(idx, &sample, multiple);
                let signature = format!("{background}\n{attributes}");
                if current.contains(&signature) {
                    continue;
                }
                if !self.last[idx].contains(&signature) {
                    self.push(ScanLine { text: background, swatch: None });
                    self.push(ScanLine { text: attributes, swatch: Some((sample.fg, sample.bg)) });
                    changed = true;
                }
                current.push(signature);
            }
            self.last[idx] = current;
        }

        for outcome in &run.outcomes {
            match outcome {
                EventOutcome::Fired { event } => {
                    let Some(text_event) = events.get(*event) else { continue };
                    let Some(action) = &text_event.action else { continue };
                    self.push(ScanLine { text: format!("FIRE {} {}", event_label(*event, text_event), action_label(action)), swatch: None });
                    self.fires += 1;
                    changed = true;
                }
                EventOutcome::Skipped { event, by } => {
                    let target_label =
                        events.get(*event).map(|text_event| event_label(*event, text_event)).unwrap_or_else(|| format!("#{}", event + 1));
                    let guard_label = events.get(*by).map(|text_event| event_label(*by, text_event)).unwrap_or_else(|| format!("#{}", by + 1));
                    self.push(ScanLine { text: format!("SKIP {target_label} <- {guard_label}"), swatch: None });
                    self.skips += 1;
                    changed = true;
                }
            }
        }

        let title = if events.len() == 1 {
            format!("SCAN \"{}\" [{}] fire={} skip={}", events[0].input.content, live, self.fires, self.skips)
        } else {
            format!("SCAN {} events [{}] fire={} skip={}", events.len(), live, self.fires, self.skips)
        };
        if title != self.title {
            self.title = title;
            changed = true;
        }
        if live != self.live {
            self.live = live;
            changed = true;
        }
        changed
    }

    /// Whether the debug overlay should be shown.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Overlay title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Rolling log lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &ScanLine> {
        self.lines.iter()
    }

    fn push(&mut self, line: ScanLine) {
        self.lines.push_back(line);
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }
}

fn background_line(index: usize, sample: &ScanSample, multiple: bool) -> String {
    let colors: Vec<String> = sample.backgrounds.iter().map(|color| color.to_yaml()).collect();
    format!("{}background: [{}]", prefix(index, multiple), colors.join(", "))
}

fn attribute_line(index: usize, sample: &ScanSample, multiple: bool) -> String {
    let match_state = match sample.background_ok {
        None => "any",
        Some(true) => "ok",
        Some(false) => "no",
    };
    format!(
        "{}fg={} bg={} B{}D{}I{}U{}R{} {}",
        prefix(index, multiple),
        ColorSpec::from_terminal(sample.fg).display(),
        ColorSpec::from_terminal(sample.bg).display(),
        u8::from(sample.bold),
        u8::from(sample.dim),
        u8::from(sample.italic),
        u8::from(sample.underline),
        u8::from(sample.inverse),
        match_state
    )
}

fn prefix(index: usize, multiple: bool) -> String {
    if multiple { format!("#{} ", index + 1) } else { String::new() }
}

fn event_label(index: usize, event: &TextEvent) -> String {
    match event.id.as_deref() {
        Some(id) => id.to_string(),
        None => format!("#{}", index + 1),
    }
}

fn action_label(action: &EventAction) -> String {
    let keys = action.keystroke.iter().map(|keystroke| keystroke.label()).collect::<Vec<_>>().join("+");
    match (&action.sound, keys.is_empty()) {
        (Some(sound), true) => format!("sound={sound}"),
        (Some(sound), false) => format!("{keys} sound={sound}"),
        (None, false) => keys,
        (None, true) => "no action".to_string(),
    }
}
