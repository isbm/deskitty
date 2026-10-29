mod audio;
mod clidef;
mod compositor;
mod config;
mod events;
mod palette;
mod proc;
mod pty;
mod scan;
mod sounds;
mod sysinfo;
mod terminal;
mod workspace;

#[cfg(test)]
mod compositor_ut;
#[cfg(test)]
mod config_ut;
#[cfg(test)]
mod events_ut;
#[cfg(test)]
mod scan_ut;
#[cfg(test)]
mod sounds_ut;
#[cfg(test)]
mod terminal_ut;

use compositor::{MenuOptions, MenuState, Mode, Notice};
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
    let mut actions_stopped = false;
    let mut show_desktop = cfg.show_desktop;
    let (mut grid_rows, mut grid_cols) = grid_dimensions(size.width, size.height, show_desktop);

    let mut ws = workspace::Workspace::spawn_all(&cfg.tabs, grid_rows, grid_cols).map_err(|e| e.to_string())?;
    let mut scan = scan::ScanState::default();
    let mut sounds = audio::SoundPlayer::new();
    let mut mode = match sounds.reason() {
        Some(reason) if !cfg.debug && cfg.events.uses_sounds() => Mode::Notice(Notice::new(
            "Sounds disabled",
            vec![
                format!("Sounds are disabled: {reason}."),
                "Install paplay, pw-play, aplay, or ffplay".to_string(),
                "to enable action sounds.".to_string(),
            ],
        )),
        _ => Mode::Normal,
    };
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

            let action_mode = if cfg.debug {
                workspace::ActionMode::Debug
            } else if actions_stopped {
                workspace::ActionMode::Paused
            } else {
                workspace::ActionMode::Active
            };
            let (run, disappear_run) = if matches!(&mode, Mode::Normal) {
                let run = ws.apply_events(&cfg.events.on_text, action_mode).map_err(|e| e.to_string())?;
                let disappear_run = ws.apply_disappear_events(&cfg.events.on_disappear, action_mode).map_err(|e| e.to_string())?;
                (run, disappear_run)
            } else {
                (events::EventRun::default(), events::DisappearRun::default())
            };
            if cfg.debug && scan.update(&cfg.events.on_text, ws.active_tab().terminal.screen(), &run) {
                dirty = true;
            }
            if action_mode == workspace::ActionMode::Active {
                for outcome in &run.outcomes {
                    if let events::EventOutcome::Fired { event } = outcome
                        && let Some(action) = &cfg.events.on_text[*event].action
                        && let Some(sound) = &action.sound
                    {
                        sounds.play(sound);
                    }
                }
                for &event in &disappear_run.fired {
                    if let Some(action) = &cfg.events.on_disappear[event].action
                        && let Some(sound) = &action.sound
                    {
                        sounds.play(sound);
                    }
                }
            }
            ws.tick_actions(Instant::now(), action_mode != workspace::ActionMode::Active).map_err(|e| e.to_string())?;

            if crossterm::event::poll(tick).map_err(|e| e.to_string())? {
                match crossterm::event::read().map_err(|e| e.to_string())? {
                    Event::Key(key) => match &mut mode {
                        Mode::Normal => {
                            if key.code == KeyCode::Char('4') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                let options = MenuOptions { actions_stopped, show_desktop, actions_enabled: !cfg.debug };
                                let menu = MenuState::new(&ws.labels(), options);
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
                                compositor::MenuItem::ToggleActions { enabled: false, .. } => {}
                                compositor::MenuItem::ToggleActions { .. } => {
                                    actions_stopped = !actions_stopped;
                                    if actions_stopped {
                                        ws.clear_pending_actions();
                                    }
                                    mode = Mode::Normal;
                                    dirty = true;
                                }
                                compositor::MenuItem::ToggleDesktop { .. } => {
                                    show_desktop = !show_desktop;
                                    let size = term.size().map_err(|e| e.to_string())?;
                                    let (rows, cols) = grid_dimensions(size.width, size.height, show_desktop);
                                    grid_rows = rows;
                                    grid_cols = cols;
                                    ws.resize_all(grid_rows, grid_cols).map_err(|e| e.to_string())?;
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
                                        compositor::MenuItem::ToggleActions { enabled: false, .. } => {}
                                        compositor::MenuItem::ToggleActions { .. } => {
                                            actions_stopped = !actions_stopped;
                                            if actions_stopped {
                                                ws.clear_pending_actions();
                                            }
                                            mode = Mode::Normal;
                                            dirty = true;
                                        }
                                        compositor::MenuItem::ToggleDesktop { .. } => {
                                            show_desktop = !show_desktop;
                                            let size = term.size().map_err(|e| e.to_string())?;
                                            let (rows, cols) = grid_dimensions(size.width, size.height, show_desktop);
                                            grid_rows = rows;
                                            grid_cols = cols;
                                            ws.resize_all(grid_rows, grid_cols).map_err(|e| e.to_string())?;
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
                        Mode::Notice(_) => match key.code {
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => {
                                mode = Mode::Normal;
                                dirty = true;
                            }
                            _ => {}
                        },
                    },
                    Event::Resize(cols, rows) => {
                        let (new_rows, new_cols) = grid_dimensions(cols, rows, show_desktop);
                        grid_rows = new_rows;
                        grid_cols = new_cols;
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
                    compositor::render(
                        f,
                        ws.active_tab().terminal.screen(),
                        &ws.labels(),
                        ws.active_idx(),
                        &mode,
                        info.as_ref(),
                        bg_active,
                        show_desktop,
                        &scan,
                    );
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

fn grid_dimensions(width: u16, height: u16, show_desktop: bool) -> (u16, u16) {
    if show_desktop {
        (height.saturating_sub(3), width.saturating_sub(2))
    } else {
        (height, width)
    }
}
