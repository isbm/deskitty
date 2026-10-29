use crate::events::{DisappearEvent, DisappearRun, DisappearStatus, DisappearTrace, EventOutcome, EventRun, EventsConfig, TextEvent};
use crate::scan::ScanState;

fn events(yaml: &str) -> Vec<TextEvent> {
    let config: EventsConfig = serde_yaml::from_str(yaml).expect("valid event config");
    config.on_text
}

fn disappear_events(yaml: &str) -> Vec<DisappearEvent> {
    let config: EventsConfig = serde_yaml::from_str(yaml).expect("valid event config");
    config.on_disappear
}

fn update_text(scan: &mut ScanState, events: &[TextEvent], screen: &vt100::Screen, run: &EventRun) -> bool {
    scan.update(events, &[], screen, run, &DisappearRun::default())
}

fn fired(event: usize) -> EventRun {
    EventRun { outcomes: vec![EventOutcome::Fired { event }] }
}

fn screen(sgr: &str, text: &str) -> vt100::Parser {
    let mut parser = vt100::Parser::new(4, 80, 0);
    parser.process(format!("\x1b[{sgr}m{text}\x1b[0m").as_bytes());
    parser
}

#[test]
fn logs_copy_paste_background_and_attributes() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
    background: [green]
  action:
    keystroke: ENTER
"#,
    );
    let parser = screen("48;2;0;95;0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &EventRun::default()));
    assert!(scan.active());
    assert!(scan.title().contains("SCAN \"foobar\" [1]"));

    let lines: Vec<String> = scan.lines().map(|line| line.text.clone()).collect();
    assert!(lines.iter().any(|line| line == "background: [\"#005f00\"]"));
    assert!(lines.iter().any(|line| line.contains("bg=#005f00") && line.ends_with("ok")));
}

#[test]
fn deduplicates_unchanged_samples() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
    background: [green]
  action:
    keystroke: ENTER
"#,
    );
    let parser = screen("48;2;0;95;0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &EventRun::default()));
    let count = scan.lines().count();
    assert!(!update_text(&mut scan, &events, parser.screen(), &EventRun::default()));
    assert_eq!(scan.lines().count(), count);
}

#[test]
fn logs_background_changes() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
    background: [green]
  action:
    keystroke: ENTER
"#,
    );
    let green = screen("48;2;0;95;0", "foobar");
    let default = screen("0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, green.screen(), &EventRun::default()));
    assert!(update_text(&mut scan, &events, default.screen(), &EventRun::default()));

    let lines: Vec<String> = scan.lines().map(|line| line.text.clone()).collect();
    assert!(lines.iter().any(|line| line == "background: [\"#005f00\"]"));
    assert!(lines.iter().any(|line| line == "background: [\"!default\"]"));
    assert!(lines.iter().any(|line| line.ends_with("no")));
}

#[test]
fn logs_fired_events() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
    background: [green]
  action:
    keystroke: ENTER
"#,
    );
    let parser = screen("48;2;0;95;0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &fired(0)));
    assert!(scan.title().contains("fire=1"));
    assert!(scan.lines().any(|line| line.text == "FIRE #1 ENTER"));
}

#[test]
fn fire_log_includes_sound() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
  action:
    keystroke: [enter]
    sound: accept
"#,
    );
    let parser = screen("0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &fired(0)));
    assert!(scan.lines().any(|line| line.text == "FIRE #1 ENTER sound=accept"));
}

#[test]
fn logs_skip_outcomes() {
    let events = events(
        r#"
on_text:
  - id: "deny-script"
    input:
      content: "python3"
    action:
      keystroke: [esc]
  - id: "allow-once"
    input:
      content: "Allow once"
    action:
      keystroke: [enter]
"#,
    );
    let parser = screen("0", "nothing");
    let mut scan = ScanState::default();
    let run = EventRun { outcomes: vec![EventOutcome::Skipped { event: 1, by: 0 }] };

    assert!(update_text(&mut scan, &events, parser.screen(), &run));
    assert!(scan.title().contains("skip=1"));
    assert!(scan.lines().any(|line| line.text == "SKIP allow-once <- deny-script"));
}

#[test]
fn prefixes_multiple_event_samples() {
    let events = events(
        r#"
on_text:
  - input:
      content: "foobar"
    action:
      keystroke: ENTER
  - input:
      content: "baz"
    action:
      keystroke: ESC
"#,
    );
    let parser = screen("48;2;0;95;0", "foobar baz");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &EventRun::default()));
    assert!(scan.title().contains("SCAN 2 events"));

    let lines: Vec<String> = scan.lines().map(|line| line.text.clone()).collect();
    assert!(lines.iter().any(|line| line.starts_with("#1 background:")));
    assert!(lines.iter().any(|line| line.starts_with("#2 background:")));
}

#[test]
fn shows_zero_matches_without_clearing_history() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
  action:
    keystroke: ENTER
"#,
    );
    let present = screen("0", "foobar");
    let absent = screen("0", "nothing");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, present.screen(), &EventRun::default()));
    let count = scan.lines().count();
    assert!(update_text(&mut scan, &events, absent.screen(), &EventRun::default()));
    assert!(scan.title().contains("[0]"));
    assert_eq!(scan.lines().count(), count);
}

#[test]
fn deactivates_when_there_are_no_events() {
    let events = events(
        r#"
on_text:
  input:
    content: "foobar"
  action:
    keystroke: ENTER
"#,
    );
    let parser = screen("0", "foobar");
    let mut scan = ScanState::default();

    assert!(update_text(&mut scan, &events, parser.screen(), &EventRun::default()));
    assert!(scan.active());
    assert!(scan.update(&[], &[], parser.screen(), &EventRun::default(), &DisappearRun::default()));
    assert!(!scan.active());
}

#[test]
fn logs_disappear_fire_outcomes() {
    let events = disappear_events(
        r#"
on_disappear:
  input:
    content: "Thinking ..."
  timeout: 10
  action:
    sound: tada
"#,
    );
    let parser = screen("0", "nothing");
    let mut scan = ScanState::default();
    let run = DisappearRun { traces: vec![DisappearTrace { event: 0, status: DisappearStatus::Fired }] };

    assert!(scan.update(&[], &events, parser.screen(), &EventRun::default(), &run));
    assert!(scan.title().contains("SCAN disappear"));
    assert!(scan.lines().any(|line| line.text == "FIRE #1 sound=tada"));
}

#[test]
fn logs_disappear_lifecycle() {
    let events = disappear_events(
        r#"
on_disappear:
  id: "agent-finished"
  input:
    content: "Thinking ..."
  timeout: 10
  action:
    sound: tada
"#,
    );
    let parser = screen("0", "nothing");
    let mut scan = ScanState::default();
    let run = DisappearRun {
        traces: vec![
            DisappearTrace { event: 0, status: DisappearStatus::Tracking },
            DisappearTrace { event: 0, status: DisappearStatus::Timeout },
            DisappearTrace { event: 0, status: DisappearStatus::Cancelled },
            DisappearTrace { event: 0, status: DisappearStatus::Fired },
        ],
    };

    assert!(scan.update(&[], &events, parser.screen(), &EventRun::default(), &run));

    let lines: Vec<String> = scan.lines().map(|line| line.text.clone()).collect();
    assert!(lines.iter().any(|line| line == "WAIT agent-finished \"Thinking ...\""));
    assert!(lines.iter().any(|line| line == "TRACK agent-finished"));
    assert!(lines.iter().any(|line| line == "TIMEOUT agent-finished 10s"));
    assert!(lines.iter().any(|line| line == "CANCEL agent-finished"));
    assert!(lines.iter().any(|line| line == "FIRE agent-finished sound=tada"));
    assert!(scan.title().contains("fire=1"));
}
