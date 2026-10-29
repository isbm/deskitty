mod clidef;
mod compositor;
mod config;
mod events;
mod palette;
mod proc;
mod pty;
mod sounds;
mod sysinfo;
mod terminal;
mod workspace;

#[cfg(test)]
mod config_ut;
#[cfg(test)]
mod events_ut;
#[cfg(test)]
mod sounds_ut;
#[cfg(test)]
mod terminal_ut;

use compositor::{MenuState, Mode};
use crossterm::ExecutableCommand;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use std::io;
use std::time::{Duration, Instant};

fn main() {
    if let Err(err) = run() {
        eprintln!("deskitty: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut cli = clidef::cli(env!("CARGO_PKG_VERSION"));
    let matches = cli.clone().get_matches();

    if matches.get_flag("help") {
        cli.print_help().map_err(|err| err.to_string())?;
        println!();
        return Ok(());
    }

    let cfg_path = matches.get_one::<String>("config").map(|s| s.as_str());
    let cfg = config::load(cfg_path).map_err(|err| format!("{err:#}"))?;

    let mut stdout = io::stdout();
    stdout.execute(crossterm::terminal::EnterAlternateScreen).map_err(|e| e.to_string())?;
    crossterm::terminal::enable_raw_mode().map_err(|e| e.to_string())?;

    let backend = CrosstermBackend::new(stdout);
    let mut term = ratatui::Terminal::new(backend).map_err(|e| e.to_string())?;

    let size = term.size().map_err(|e| e.to_string())?;
    let mut grid_rows = size.height.saturating_sub(3);
    let mut grid_cols = size.width.saturating_sub(2);

    let mut ws = workspace::Workspace::spawn_all(&cfg.tabs, grid_rows, grid_cols).map_err(|e| e.to_string())?;

    let mut mode = Mode::Normal;
    let mut dirty = true;
    let mut prev_bg_active = false;
    let tick = Duration::from_millis(16);

    let result: Result<(), String> = (|| {
        'main: loop {
            if ws.poll_all().map_err(|e| e.to_string())? {
                dirty = true;
            }

            let exited = ws.exited_tabs().map_err(|e| e.to_string())?;
            if !exited.is_empty() {
                for &idx in &exited {
                    ws.close_tab(idx);
                }
                if ws.all_dead() {
                    break 'main Ok(());
                }
                if ws.is_dead(ws.active_idx()) {
                    for i in 0..ws.labels().len() {
                        if !ws.is_dead(i) {
                            ws.switch_to(i);
                            break;
                        }
                    }
                }
                mode = Mode::Normal;
                dirty = true;
            }

            let action_mode = workspace::ActionMode::Active;
            if matches!(&mode, Mode::Normal) {
                ws.apply_events(&cfg.events.on_text, action_mode).map_err(|e| e.to_string())?;
            }
            ws.tick_actions(Instant::now(), action_mode != workspace::ActionMode::Active).map_err(|e| e.to_string())?;

            if crossterm::event::poll(tick).map_err(|e| e.to_string())? {
                match crossterm::event::read().map_err(|e| e.to_string())? {
                    Event::Key(key) => match &mut mode {
                        Mode::Normal => {
                            if key.code == KeyCode::Char('4') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                let menu = MenuState::new(&ws.labels());
                                mode = Mode::Command(menu);
                                dirty = true;
                            } else if let Some(bytes) = compositor::key_to_bytes(&key, ws.active_tab().terminal.application_cursor_keys())
                                && let Some(ref mut pty) = ws.active_tab_mut().pty
                            {
                                pty.write(&bytes).map_err(|e| e.to_string())?;
                                dirty = true;
                            }
                        }
                        Mode::Command(menu) => match key.code {
                            KeyCode::Esc => {
                                mode = Mode::Normal;
                                dirty = true;
                            }
                            KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                mode = Mode::Normal;
                                dirty = true;
                            }
                            KeyCode::Up => {
                                menu.prev();
                                dirty = true;
                            }
                            KeyCode::Down => {
                                menu.next();
                                dirty = true;
                            }
                            KeyCode::Enter => match &menu.items[menu.selected] {
                                compositor::MenuItem::Tab { idx, .. } => {
                                    if ws.is_dead(*idx) {
                                        ws.relaunch(*idx, grid_rows, grid_cols).map_err(|e| e.to_string())?;
                                    }
                                    ws.switch_to(*idx);
                                    mode = Mode::Normal;
                                    dirty = true;
                                }
                                compositor::MenuItem::Quit => {
                                    break 'main Ok(());
                                }
                            },
                            KeyCode::Char(c) => {
                                if let Some(idx) = menu.find_by_shortcut(c) {
                                    match &menu.items[idx] {
                                        compositor::MenuItem::Tab { idx: tab_idx, .. } => {
                                            if ws.is_dead(*tab_idx) {
                                                ws.relaunch(*tab_idx, grid_rows, grid_cols).map_err(|e| e.to_string())?;
                                            }
                                            ws.switch_to(*tab_idx);
                                            mode = Mode::Normal;
                                            dirty = true;
                                        }
                                        compositor::MenuItem::Quit => {
                                            break 'main Ok(());
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                    },
                    Event::Resize(cols, rows) => {
                        grid_rows = rows.saturating_sub(3);
                        grid_cols = cols.saturating_sub(2);
                        ws.resize_all(grid_rows, grid_cols).map_err(|e| e.to_string())?;
                        term.resize(Rect::new(0, 0, cols, rows)).map_err(|e| e.to_string())?;
                        dirty = true;
                    }
                    _ => {}
                }
            }

            let bg_active = ws.background_active(ws.active_idx());
            if bg_active != prev_bg_active {
                prev_bg_active = bg_active;
                dirty = true;
            }

            if dirty {
                let info = sysinfo::gather();
                term.draw(|f| {
                    compositor::render(f, ws.active_tab().terminal.screen(), &ws.labels(), ws.active_idx(), &mode, info.as_ref(), bg_active);
                })
                .map_err(|e| e.to_string())?;
                dirty = false;
            }
        }
    })();

    crossterm::terminal::disable_raw_mode().map_err(|e| e.to_string())?;
    io::stdout().execute(crossterm::terminal::LeaveAlternateScreen).map_err(|e| e.to_string())?;

    drop(ws);

    result
}
