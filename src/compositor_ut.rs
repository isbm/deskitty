use crate::compositor::{MenuItem, MenuOptions, MenuState};

fn options() -> MenuOptions {
    MenuOptions { actions_stopped: false, show_desktop: true, actions_enabled: true }
}

#[test]
fn menu_orders_tabs_toggles_and_quit() {
    let labels = vec![('t', "Editor".to_string())];
    let menu = MenuState::new(&labels, options());

    assert_eq!(menu.items.len(), 4);
    assert!(matches!(menu.items[0], MenuItem::Tab { .. }));
    assert!(matches!(menu.items[1], MenuItem::ToggleActions { stopped: false, enabled: true }));
    assert!(matches!(menu.items[2], MenuItem::ToggleDesktop { visible: true }));
    assert!(matches!(menu.items[3], MenuItem::Quit));
}

#[test]
fn disables_actions_toggle_in_debug_mode() {
    let labels = vec![('t', "Editor".to_string())];
    let menu = MenuState::new(&labels, MenuOptions { actions_enabled: false, ..options() });

    assert_eq!(menu.items.len(), 4);
    assert!(matches!(menu.items[1], MenuItem::ToggleActions { enabled: false, .. }));
    assert!(!menu.items[1].selectable());
    assert_eq!(menu.find_by_shortcut('a'), None);
}

#[test]
fn navigation_skips_disabled_actions_toggle() {
    let labels = vec![('t', "Editor".to_string())];
    let mut menu = MenuState::new(&labels, MenuOptions { actions_enabled: false, ..options() });

    menu.next();
    assert_eq!(menu.selected, 2);

    menu.prev();
    assert_eq!(menu.selected, 0);
}

#[test]
fn finds_toggle_shortcuts() {
    let labels = vec![('t', "Editor".to_string())];
    let menu = MenuState::new(&labels, MenuOptions { actions_stopped: true, show_desktop: false, ..options() });

    assert_eq!(menu.find_by_shortcut('a'), Some(1));
    assert_eq!(menu.find_by_shortcut('d'), Some(2));
    assert_eq!(menu.find_by_shortcut('q'), Some(3));
}
