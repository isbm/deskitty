use crate::compositor::{MenuItem, MenuOptions, MenuState, key_to_bytes};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
