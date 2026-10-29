use crate::compositor::{MenuItem, MenuOptions, MenuState};

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
