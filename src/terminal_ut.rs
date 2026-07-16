use crate::terminal::Terminal;

#[test]
fn translates_g0_dec_vertical_line() {
    let mut terminal = Terminal::new(1, 3);

    terminal.process(b"A\x1b(0x\x1b(BB");

    assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
    assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
    assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
}

#[test]
fn translates_g1_dec_vertical_line() {
    let mut terminal = Terminal::new(1, 3);

    terminal.process(b"A\x1b)0\x0ex\x0fB");

    assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
    assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
    assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
}

#[test]
fn preserves_utf8_vertical_line() {
    let mut terminal = Terminal::new(1, 3);

    terminal.process("A│B".as_bytes());

    assert_eq!(terminal.screen().cell(0, 0).unwrap().contents(), "A");
    assert_eq!(terminal.screen().cell(0, 1).unwrap().contents(), "│");
    assert_eq!(terminal.screen().cell(0, 2).unwrap().contents(), "B");
}

#[test]
fn does_not_translate_csi_while_dec_graphics_is_active() {
    let mut terminal = Terminal::new(1, 2);

    terminal.process(b"\x1b(0\x1b[31mx");

    let cell = terminal.screen().cell(0, 0).unwrap();
    assert_eq!(cell.contents(), "│");
    assert_eq!(cell.fgcolor(), vt100::Color::Idx(1));
}

#[test]
fn normalizes_hvp_to_cup() {
    let mut terminal = Terminal::new(3, 3);

    terminal.process(b"\x1b[2;3fX");

    assert_eq!(terminal.screen().cell(1, 2).unwrap().contents(), "X");
}

#[test]
fn normalizes_hvp_while_dec_graphics_is_active() {
    let mut terminal = Terminal::new(3, 3);

    terminal.process(b"\x1b(0\x1b[2;3fx");

    assert_eq!(terminal.screen().cell(1, 2).unwrap().contents(), "│");
}
