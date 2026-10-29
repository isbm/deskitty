use crate::events::{ColorSpec, DisappearGate, EventOutcome, EventsConfig, Keystroke, MatchGate, NamedColor, SETTLE, evaluate_events};
use std::time::{Duration, Instant};

const EVENT_YAML: &str = r#"
on_text:
  input:
    content: "1. Allow once"
    background:
      - green
      - !default
      - !black
  action:
    keystroke: ENTER
"#;

fn parse_events(background: &str) -> EventsConfig {
    let yaml = format!(
        r#"
on_text:
  input:
    content: "1. Allow once"
    background: {background}
  action:
    keystroke: ENTER
"#
    );
    serde_yaml::from_str(&yaml).expect("valid event config")
}

fn screen(sgr: &str, text: &str) -> vt100::Parser {
    let mut parser = vt100::Parser::new(4, 80, 0);
    parser.process(format!("\x1b[{sgr}m{text}\x1b[0m").as_bytes());
    parser
}

#[test]
fn parses_single_event_with_tagged_colors() {
    let events: EventsConfig = serde_yaml::from_str(EVENT_YAML).expect("valid event config");

    assert_eq!(events.on_text.len(), 1);
    assert_eq!(events.on_text[0].input.content, "1. Allow once");
    assert_eq!(
        events.on_text[0].input.background,
        vec![ColorSpec::Named(NamedColor::Green), ColorSpec::Default, ColorSpec::Named(NamedColor::Black)]
    );
}

#[test]
fn parses_event_list() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - input:
      content: "1. Allow once"
    action:
      keystroke: ENTER
  - input:
      content: "2. No, suggest changes"
    action:
      keystroke: ESC
"#,
    )
    .expect("valid event config");

    assert_eq!(events.on_text.len(), 2);
}

#[test]
fn parses_hex_and_index_colors() {
    let events = parse_events(r##"["#005f00", 22]"##);

    assert_eq!(events.on_text[0].input.background, vec![ColorSpec::Rgb(0, 95, 0), ColorSpec::Ansi(22)]);
}

#[test]
fn parses_supported_keystrokes() {
    let enter: Keystroke = serde_yaml::from_str("ENTER").expect("enter is supported");
    assert_eq!(enter.to_bytes(false), Some(b"\r".to_vec()));

    let ctrl_c: Keystroke = serde_yaml::from_str("CTRL+C").expect("ctrl+c is supported");
    assert_eq!(ctrl_c.to_bytes(false), Some(vec![0x03]));

    let f1: Keystroke = serde_yaml::from_str("F1").expect("f1 is supported");
    assert_eq!(f1.to_bytes(false), Some(b"\x1bOP".to_vec()));
}

#[test]
fn rejects_unknown_keystrokes() {
    assert!(serde_yaml::from_str::<Keystroke>("NOT_A_KEY").is_err());
}

#[test]
fn matches_truecolor_green_theme_background() {
    let events = parse_events("[green]");
    let parser = screen("48;2;0;95;0", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn matches_256_color_green() {
    let events = parse_events("[green]");
    let parser = screen("48;5;22", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn matches_ansi_green() {
    let events = parse_events("[green]");
    let parser = screen("42", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn rejects_wrong_background() {
    let events = parse_events("[green]");
    let parser = screen("48;2;200;0;0", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 0);
}

#[test]
fn matches_default_background() {
    let events = parse_events(r#"["!default"]"#);
    let parser = screen("0", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn matches_black_background() {
    let events = parse_events(r#"["!black"]"#);
    let parser = screen("48;2;0;0;0", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn accepts_single_background_value() {
    let events = parse_events("green");

    assert_eq!(events.on_text[0].input.background, vec![ColorSpec::Named(NamedColor::Green)]);
}

#[test]
fn inverse_uses_foreground_as_effective_background() {
    let events = parse_events("[green]");
    let parser = screen("32;7", "1. Allow once");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn samples_truecolor_background_and_match_state() {
    let events = parse_events("[green]");
    let parser = screen("48;2;0;95;0", "1. Allow once");

    let samples = events.on_text[0].scan_samples(parser.screen());
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].backgrounds, vec![ColorSpec::Rgb(0, 95, 0)]);
    assert_eq!(samples[0].background_ok, Some(true));
}

#[test]
fn samples_default_background_as_mismatch() {
    let events = parse_events("[green]");
    let parser = screen("0", "1. Allow once");

    let samples = events.on_text[0].scan_samples(parser.screen());
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].backgrounds, vec![ColorSpec::Default]);
    assert_eq!(samples[0].background_ok, Some(false));
}

#[test]
fn samples_each_occurrence_separately() {
    let events = parse_events("[green]");
    let mut parser = vt100::Parser::new(4, 80, 0);
    parser.process(b"\x1b[42m1. Allow once\x1b[0m\n\x1b[0m1. Allow once\x1b[0m");

    let samples = events.on_text[0].scan_samples(parser.screen());
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].background_ok, Some(true));
    assert_eq!(samples[1].background_ok, Some(false));
}

#[test]
fn formats_copy_paste_colors() {
    assert_eq!(ColorSpec::from_terminal(vt100::Color::Rgb(0, 95, 0)).to_yaml(), "\"#005f00\"");
    assert_eq!(ColorSpec::from_terminal(vt100::Color::Idx(22)).to_yaml(), "\"#005f00\"");
    assert_eq!(ColorSpec::from_terminal(vt100::Color::Default).to_yaml(), "\"!default\"");
}

#[test]
fn labels_keystrokes_for_debug() {
    let enter: Keystroke = serde_yaml::from_str("ENTER").expect("enter is supported");
    assert_eq!(enter.label(), "ENTER");

    let ctrl_c: Keystroke = serde_yaml::from_str("CTRL+C").expect("ctrl+c is supported");
    assert_eq!(ctrl_c.label(), "CTRL+C");
}

#[test]
fn counts_multiple_occurrences() {
    let events = parse_events("[green]");
    let mut parser = vt100::Parser::new(4, 80, 0);
    parser.process(b"\x1b[42m1. Allow once\x1b[0m\n\x1b[42m1. Allow once\x1b[0m");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 2);
}

#[test]
fn matches_across_wrapped_rows() {
    let events = parse_events("[green]");
    let mut parser = vt100::Parser::new(4, 10, 0);
    parser.process(b"\x1b[42m1. Allow once\x1b[0m");

    assert_eq!(events.on_text[0].match_count(parser.screen()), 1);
}

#[test]
fn stable_match_fires_once() {
    let mut gate = MatchGate::default();
    let start = Instant::now();

    assert!(!gate.update(0, start));
    assert!(!gate.update(1, start + Duration::from_millis(16)));

    let settled = start + Duration::from_millis(16) + SETTLE;
    assert!(gate.update(1, settled));
    assert!(!gate.update(1, settled + SETTLE));
    assert!(!gate.update(1, settled + SETTLE + SETTLE));
}

#[test]
fn new_occurrence_fires_once_more() {
    let mut gate = MatchGate::default();
    let start = Instant::now();

    assert!(!gate.update(1, start));
    assert!(gate.update(1, start + SETTLE));

    let second = start + SETTLE + Duration::from_millis(16);
    assert!(!gate.update(2, second));
    assert!(gate.update(2, second + SETTLE));
    assert!(!gate.update(2, second + SETTLE + SETTLE));
}

#[test]
fn count_jump_fires_only_once() {
    let mut gate = MatchGate::default();
    let start = Instant::now();

    assert!(!gate.update(2, start));
    assert!(gate.update(2, start + SETTLE));
    assert!(!gate.update(2, start + SETTLE + SETTLE));
}

#[test]
fn transient_drop_does_not_rearm() {
    let mut gate = MatchGate::default();
    let start = Instant::now();

    assert!(!gate.update(1, start));
    assert!(gate.update(1, start + SETTLE));

    let drop = start + SETTLE + Duration::from_millis(16);
    assert!(!gate.update(0, drop));
    assert!(!gate.update(1, drop + Duration::from_millis(100)));
    assert!(!gate.update(1, drop + Duration::from_millis(100) + SETTLE));
}

#[test]
fn stable_clear_rearms() {
    let mut gate = MatchGate::default();
    let start = Instant::now();

    assert!(!gate.update(1, start));
    assert!(gate.update(1, start + SETTLE));

    let clear = start + SETTLE + Duration::from_millis(16);
    assert!(!gate.update(0, clear));
    assert!(!gate.update(0, clear + SETTLE));

    let next = clear + SETTLE + Duration::from_millis(16);
    assert!(!gate.update(1, next));
    assert!(gate.update(1, next + SETTLE));
}

#[test]
fn parses_keystroke_list_and_delay() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke:
      - right
      - enter
    delay: 0.1
"#,
    )
    .expect("valid event config");

    let action = events.on_text[0].action.as_ref().expect("action");
    assert_eq!(action.keystroke.len(), 2);
    assert_eq!(action.delay, Duration::from_millis(100));
}

#[test]
fn parses_integer_delay() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [enter]
    delay: 1
"#,
    )
    .expect("valid event config");

    let action = events.on_text[0].action.as_ref().expect("action");
    assert_eq!(action.delay, Duration::from_secs(1));
}

#[test]
fn rejects_negative_delay() {
    let result: Result<EventsConfig, _> = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [enter]
    delay: -0.1
"#,
    );

    assert!(result.is_err());
}

#[test]
fn parses_optional_action_and_skip_targets() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "deny-script"
    input:
      content: "python3"
    skip: ["allow-once"]
  - id: "allow-once"
    input:
      content: "Allow once"
    action:
      keystroke: [enter]
"#,
    )
    .expect("valid event config");

    assert!(events.on_text[0].action.is_none());
    assert_eq!(events.on_text[0].skip, vec!["allow-once"]);
    events.validate().expect("valid rule targets");
}

#[test]
fn parses_scalar_skip_target() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "deny-script"
    input: { content: "python3" }
    skip: "allow-once"
  - id: "allow-once"
    input: { content: "Allow once" }
"#,
    )
    .expect("valid event config");

    assert_eq!(events.on_text[0].skip, vec!["allow-once"]);
    events.validate().expect("valid rule targets");
}

#[test]
fn rejects_duplicate_ids() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "same"
    input: { content: "one" }
  - id: "same"
    input: { content: "two" }
"#,
    )
    .expect("valid yaml");

    assert!(events.validate().is_err());
}

#[test]
fn rejects_unknown_skip_target() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "guard"
    input: { content: "one" }
    skip: ["missing"]
"#,
    )
    .expect("valid yaml");

    assert!(events.validate().is_err());
}

#[test]
fn rejects_skip_target_above_guard() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "allow-once"
    input: { content: "Allow once" }
  - id: "deny-script"
    input: { content: "python3" }
    skip: ["allow-once"]
"#,
    )
    .expect("valid yaml");

    assert!(events.validate().is_err());
}

#[test]
fn earlier_guard_skips_later_action() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "deny-script"
    input: { content: "python3" }
    skip: ["allow-once"]
    action: { keystroke: [esc] }
  - id: "allow-once"
    input: { content: "Allow once" }
    action: { keystroke: [enter] }
"#,
    )
    .expect("valid event config");
    events.validate().expect("valid rule targets");

    let mut gates = [MatchGate::default(), MatchGate::default()];
    let counts = [1, 1];
    let start = Instant::now();

    assert!(evaluate_events(&events.on_text, &counts, &mut gates, start).outcomes.is_empty());
    let run = evaluate_events(&events.on_text, &counts, &mut gates, start + SETTLE);

    assert_eq!(run.outcomes, vec![EventOutcome::Fired { event: 0 }, EventOutcome::Skipped { event: 1, by: 0 }]);
}

#[test]
fn unguarded_rules_fire_in_order() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  - id: "first"
    input: { content: "one" }
    action: { keystroke: [right] }
  - id: "second"
    input: { content: "two" }
    action: { keystroke: [enter] }
"#,
    )
    .expect("valid event config");

    let mut gates = [MatchGate::default(), MatchGate::default()];
    let counts = [1, 1];
    let start = Instant::now();

    assert!(evaluate_events(&events.on_text, &counts, &mut gates, start).outcomes.is_empty());
    let run = evaluate_events(&events.on_text, &counts, &mut gates, start + SETTLE);

    assert_eq!(run.outcomes, vec![EventOutcome::Fired { event: 0 }, EventOutcome::Fired { event: 1 }]);
}

#[test]
fn parses_sound_and_reports_usage() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [enter]
    sound: accept
"#,
    )
    .expect("valid event config");

    assert_eq!(events.on_text[0].action.as_ref().and_then(|action| action.sound.as_deref()), Some("accept"));
    assert!(events.uses_sounds());
    events.validate().expect("known sound");
}

#[test]
fn rejects_unknown_sound() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    sound: missing
"#,
    )
    .expect("valid yaml");

    assert!(events.validate().is_err());
}

#[test]
fn sound_only_action_fires() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    sound: tada
"#,
    )
    .expect("valid event config");
    events.validate().expect("known sound");

    let mut gates = [MatchGate::default()];
    let counts = [1];
    let start = Instant::now();

    assert!(evaluate_events(&events.on_text, &counts, &mut gates, start).outcomes.is_empty());
    let run = evaluate_events(&events.on_text, &counts, &mut gates, start + SETTLE);

    assert_eq!(run.outcomes, vec![EventOutcome::Fired { event: 0 }]);
}

#[test]
fn parses_repeat_interval() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [down]
    repeat: 0.5
"#,
    )
    .expect("valid event config");

    assert_eq!(events.on_text[0].action.as_ref().and_then(|action| action.repeat), Some(Duration::from_millis(500)));
}

#[test]
fn rejects_invalid_repeat_values() {
    for value in ["0", "-1", "0.0"] {
        let yaml = format!(
            r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [down]
    repeat: {value}
"#
        );
        assert!(serde_yaml::from_str::<EventsConfig>(&yaml).is_err(), "repeat {value} should fail");
    }
}

#[test]
fn repeat_gate_fires_until_cleared() {
    let mut gate = MatchGate::default();
    let interval = Duration::from_millis(500);
    let start = Instant::now();

    assert!(!gate.update_repeating(1, start, interval));

    let first = start + SETTLE;
    assert!(gate.update_repeating(1, first, interval));
    assert!(!gate.update_repeating(1, first + Duration::from_millis(100), interval));
    assert!(gate.update_repeating(1, first + interval, interval));

    let before_clear = first + interval + Duration::from_millis(100);
    assert!(!gate.update_repeating(0, before_clear, interval));
    assert!(!gate.update_repeating(0, before_clear + SETTLE, interval));

    let reappear = before_clear + SETTLE + Duration::from_millis(16);
    assert!(!gate.update_repeating(1, reappear, interval));
    assert!(gate.update_repeating(1, reappear + SETTLE, interval));
}

#[test]
fn repeat_rules_fire_while_visible() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_text:
  input:
    content: "prompt"
  action:
    keystroke: [down]
    repeat: 0.5
"#,
    )
    .expect("valid event config");

    let mut gates = [MatchGate::default()];
    let counts = [1];
    let start = Instant::now();

    assert!(evaluate_events(&events.on_text, &counts, &mut gates, start).outcomes.is_empty());

    let first = start + SETTLE;
    assert_eq!(evaluate_events(&events.on_text, &counts, &mut gates, first).outcomes, vec![EventOutcome::Fired { event: 0 }]);
    assert!(evaluate_events(&events.on_text, &counts, &mut gates, first + Duration::from_millis(100)).outcomes.is_empty());
    assert_eq!(
        evaluate_events(&events.on_text, &counts, &mut gates, first + Duration::from_millis(500)).outcomes,
        vec![EventOutcome::Fired { event: 0 }]
    );
}

#[test]
fn repeat_gate_cadence_over_time() {
    let mut gate = MatchGate::default();
    let interval = Duration::from_millis(200);
    let start = Instant::now();
    let mut now = start;
    let mut fires = 0;
    for _ in 0..200 {
        if gate.update_repeating(1, now, interval) {
            fires += 1;
        }
        now += Duration::from_millis(16);
    }
    assert!((14..=16).contains(&fires), "unexpected fire count: {fires}");
}

#[test]
fn parses_on_disappear_rule() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_disappear:
  - id: "agent-finished"
    input:
      content: "Thinking ..."
    timeout: 10
    action:
      sound: tada
"#,
    )
    .expect("valid event config");

    assert_eq!(events.on_disappear.len(), 1);
    assert_eq!(events.on_disappear[0].id.as_deref(), Some("agent-finished"));
    assert_eq!(events.on_disappear[0].timeout, Duration::from_secs(10));
    assert!(events.uses_sounds());
    events.validate().expect("valid disappear rule");
}

#[test]
fn rejects_invalid_disappear_timeout() {
    for value in ["0", "-1", "0.0"] {
        let yaml = format!(
            r#"
on_disappear:
  input:
    content: "Thinking ..."
  timeout: {value}
"#
        );
        assert!(serde_yaml::from_str::<EventsConfig>(&yaml).is_err(), "timeout {value} should fail");
    }
}

#[test]
fn rejects_repeat_on_disappear_action() {
    let events: EventsConfig = serde_yaml::from_str(
        r#"
on_disappear:
  input:
    content: "Thinking ..."
  timeout: 10
  action:
    sound: tada
    repeat: 0.5
"#,
    )
    .expect("valid yaml");

    assert!(events.validate().is_err());
}

#[test]
fn disappear_gate_fires_after_timeout_and_rearms() {
    let mut gate = DisappearGate::default();
    let timeout = Duration::from_secs(10);
    let start = Instant::now();

    assert!(!gate.update(1, start, timeout));
    assert!(!gate.update(0, start + Duration::from_secs(1), timeout));
    assert!(!gate.update(1, start + Duration::from_secs(5), timeout));
    assert!(!gate.update(0, start + Duration::from_secs(6), timeout));
    assert!(!gate.update(0, start + Duration::from_secs(15), timeout));
    assert!(gate.update(0, start + Duration::from_secs(16), timeout));
    assert!(!gate.update(0, start + Duration::from_secs(30), timeout));

    assert!(!gate.update(1, start + Duration::from_secs(31), timeout));
    assert!(!gate.update(0, start + Duration::from_secs(32), timeout));
    assert!(gate.update(0, start + Duration::from_secs(42), timeout));
}

#[test]
fn disappear_gate_ignores_unseen_content() {
    let mut gate = DisappearGate::default();
    let timeout = Duration::from_secs(1);

    assert!(!gate.update(0, Instant::now(), timeout));
    assert!(!gate.update(0, Instant::now() + Duration::from_secs(10), timeout));
}
