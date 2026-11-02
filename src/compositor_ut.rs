use crate::compositor::{MenuItem, MenuOptions, MenuState, Mode, Notice, key_to_bytes, render};
use crate::scan::ScanState;
use crate::terminal::Terminal;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};

fn options() -> MenuOptions {
    MenuOptions { actions_stopped: false, show_desktop: true }
}

#[test]
fn menu_orders_tabs_toggles_and_quit() {
    let labels = vec![('t', "Editor".to_string())];
    let menu = MenuState::new(&labels, options());

    assert_eq!(menu.items.len(), 4);
    assert!(matches!(menu.items[0], MenuItem::Tab { .. }));
    assert!(matches!(menu.items[1], MenuItem::ToggleActions { stopped: false }));
    assert!(matches!(menu.items[2], MenuItem::ToggleDesktop { visible: true }));
    assert!(matches!(menu.items[3], MenuItem::Quit));
}

#[test]
fn navigation_wraps_through_every_item() {
    let labels = vec![('t', "Editor".to_string())];
    let mut menu = MenuState::new(&labels, options());

    menu.next();
    assert_eq!(menu.selected, 1);

    menu.prev();
    assert_eq!(menu.selected, 0);

    menu.prev();
    assert_eq!(menu.selected, 3);
}

#[test]
fn finds_toggle_shortcuts() {
    let labels = vec![('t', "Editor".to_string())];
    let menu = MenuState::new(&labels, MenuOptions { actions_stopped: true, show_desktop: false });

    assert_eq!(menu.find_by_shortcut('a'), Some(1));
    assert_eq!(menu.find_by_shortcut('d'), Some(2));
    assert_eq!(menu.find_by_shortcut('q'), Some(3));
}

#[test]
fn ctrl_arrows_keep_modifiers_in_both_cursor_modes() {
    for app_cursor in [false, true] {
        for (code, expected) in [(KeyCode::Left, b"\x1b[1;5D"), (KeyCode::Right, b"\x1b[1;5C")] {
            assert_eq!(key_to_bytes(&KeyEvent::new(code, KeyModifiers::CONTROL), app_cursor), Some(expected.to_vec()));
        }
    }
}

#[test]
fn navigation_and_function_keys_encode_xterm_modifiers() {
    let keys = [
        (KeyCode::Up, "1", 'A'),
        (KeyCode::Down, "1", 'B'),
        (KeyCode::Right, "1", 'C'),
        (KeyCode::Left, "1", 'D'),
        (KeyCode::Home, "1", 'H'),
        (KeyCode::End, "1", 'F'),
        (KeyCode::Insert, "2", '~'),
        (KeyCode::Delete, "3", '~'),
        (KeyCode::PageUp, "5", '~'),
        (KeyCode::PageDown, "6", '~'),
        (KeyCode::F(1), "1", 'P'),
        (KeyCode::F(2), "1", 'Q'),
        (KeyCode::F(3), "1", 'R'),
        (KeyCode::F(4), "1", 'S'),
        (KeyCode::F(5), "15", '~'),
        (KeyCode::F(6), "17", '~'),
        (KeyCode::F(7), "18", '~'),
        (KeyCode::F(8), "19", '~'),
        (KeyCode::F(9), "20", '~'),
        (KeyCode::F(10), "21", '~'),
        (KeyCode::F(11), "23", '~'),
        (KeyCode::F(12), "24", '~'),
    ];
    for bits in 1..=7 {
        let mut modifiers = KeyModifiers::NONE;
        if bits & 1 != 0 {
            modifiers |= KeyModifiers::SHIFT;
        }
        if bits & 2 != 0 {
            modifiers |= KeyModifiers::ALT;
        }
        if bits & 4 != 0 {
            modifiers |= KeyModifiers::CONTROL;
        }
        for app_cursor in [false, true] {
            for (code, parameter, suffix) in keys {
                let expected = format!("\x1b[{parameter};{}{suffix}", bits + 1).into_bytes();
                assert_eq!(key_to_bytes(&KeyEvent::new(code, modifiers), app_cursor), Some(expected), "{code:?} {modifiers:?}");
            }
        }
    }
}

#[test]
fn unmodified_keys_preserve_csi_and_application_ss3() {
    for (code, suffix) in
        [(KeyCode::Up, 'A'), (KeyCode::Down, 'B'), (KeyCode::Right, 'C'), (KeyCode::Left, 'D'), (KeyCode::Home, 'H'), (KeyCode::End, 'F')]
    {
        for (app_cursor, prefix) in [(false, '['), (true, 'O')] {
            assert_eq!(key_to_bytes(&KeyEvent::new(code, KeyModifiers::NONE), app_cursor), Some(format!("\x1b{prefix}{suffix}").into_bytes()));
        }
    }
    for app_cursor in [false, true] {
        for (code, expected) in [
            (KeyCode::Insert, "\x1b[2~"),
            (KeyCode::Delete, "\x1b[3~"),
            (KeyCode::PageUp, "\x1b[5~"),
            (KeyCode::PageDown, "\x1b[6~"),
            (KeyCode::F(1), "\x1bOP"),
            (KeyCode::F(4), "\x1bOS"),
            (KeyCode::F(5), "\x1b[15~"),
            (KeyCode::F(12), "\x1b[24~"),
        ] {
            assert_eq!(key_to_bytes(&KeyEvent::new(code, KeyModifiers::NONE), app_cursor), Some(expected.as_bytes().to_vec()));
        }
        assert_eq!(key_to_bytes(&KeyEvent::new(KeyCode::F(13), KeyModifiers::CONTROL), app_cursor), None);
    }
}

// Capture the actual backend bytes, including Show/Hide and final MoveTo, not
// just the rendered cell buffer. A fixed viewport avoids querying a real TTY.
fn draw_cursor(terminal: &Terminal, desktop: bool, mode: &Mode, scan: &ScanState, area: Rect) -> vt100::Parser {
    let backend = CrosstermBackend::new(Vec::new());
    let mut renderer = ratatui::Terminal::with_options(backend, TerminalOptions { viewport: Viewport::Fixed(area) }).unwrap();
    renderer.draw(|frame| render(frame, terminal.screen(), &[], 0, mode, None, false, desktop, scan)).unwrap();
    let mut host = vt100::Parser::new(40, 100, 0);
    host.process(renderer.backend().writer());
    host
}

#[test]
fn cursor_stays_visible_at_pending_wrap_and_tracks_next_character() {
    for desktop in [false, true] {
        let area = if desktop { Rect::new(0, 0, 12, 7) } else { Rect::new(0, 0, 10, 4) };
        let offset = u16::from(desktop);
        let mut terminal = Terminal::new(4, 10);
        terminal.process(b"1234567890");
        assert_eq!(terminal.screen().cursor_position(), (0, 10), "vt100 pending wrap");
        let host = draw_cursor(&terminal, desktop, &Mode::Normal, &ScanState::default(), area);
        assert!(!host.screen().hide_cursor(), "visible child cursor must not become backend Hide at the right margin");
        assert_eq!(host.screen().cursor_position(), (offset, 9 + offset));
        terminal.process(b"x");
        let host = draw_cursor(&terminal, desktop, &Mode::Normal, &ScanState::default(), area);
        assert!(!host.screen().hide_cursor());
        assert_eq!(host.screen().cursor_position(), (1 + offset, 1 + offset));
    }
}

#[test]
fn cursor_tracks_typing_movement_and_child_visibility() {
    let mut terminal = Terminal::new(20, 78);
    let area = Rect::new(0, 0, 80, 23);
    for (bytes, position, hidden) in [
        (b"abc".as_slice(), (1, 4), false),
        (b"\x1b[4;7H".as_slice(), (4, 7), false),
        (b"\x1b[?25l".as_slice(), (4, 7), true),
        (b"\x1b[?25h\x1b[2D".as_slice(), (4, 5), false),
    ] {
        terminal.process(bytes);
        let host = draw_cursor(&terminal, true, &Mode::Normal, &ScanState::default(), area);
        assert_eq!(host.screen().hide_cursor(), hidden);
        if !hidden {
            assert_eq!(host.screen().cursor_position(), position);
        }
    }
}

#[test]
fn modal_windows_hide_child_cursor() {
    let terminal = Terminal::new(20, 78);
    for mode in [Mode::Command(MenuState::new(&[], options())), Mode::Notice(Notice::new("Notice", vec![]))] {
        let host = draw_cursor(&terminal, true, &mode, &ScanState::default(), Rect::new(0, 0, 80, 23));
        assert!(host.screen().hide_cursor());
    }
}

#[test]
fn scan_overlay_hides_only_the_cursor_it_covers() {
    let mut terminal = Terminal::new(20, 78);
    let events: crate::events::EventsConfig = serde_yaml::from_str("on_text:\n  input:\n    content: foobar\n").unwrap();
    let mut scan = ScanState::default();
    scan.update(&events.on_text, &[], terminal.screen(), &crate::events::EventRun::default(), &crate::events::DisappearRun::default());
    assert!(scan.active());
    for desktop in [false, true] {
        for (bytes, hidden) in [(b"\x1b[2;30H".as_slice(), true), (b"\x1b[15;3H".as_slice(), false)] {
            terminal.process(bytes);
            let host = draw_cursor(&terminal, desktop, &Mode::Normal, &scan, Rect::new(0, 0, 80, 23));
            assert_eq!(host.screen().hide_cursor(), hidden);
        }
    }
}

#[test]
fn clipped_cursor_is_not_moved_into_the_visible_grid() {
    let mut terminal = Terminal::new(20, 78);
    terminal.process(b"\x1b[2;20H");
    for area in [Rect::new(0, 0, 10, 4), Rect::new(0, 0, 0, 4), Rect::new(0, 0, 10, 0)] {
        let host = draw_cursor(&terminal, false, &Mode::Normal, &ScanState::default(), area);
        assert!(host.screen().hide_cursor());
    }
}
