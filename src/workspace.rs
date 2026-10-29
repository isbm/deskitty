use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::config::TabConfig;
use crate::events::{EventAction, EventOutcome, EventRun, Keystroke, MatchGate, TextEvent, evaluate_events};
use crate::proc::ProcStat;
use crate::pty;
use crate::terminal;

/// How event actions should be handled for the current tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMode {
    /// Execute fired actions.
    Active,
    /// Update match gates but send nothing.
    Paused,
    /// Update match gates and produce debug outcomes, but send nothing.
    Debug,
}

struct PendingAction {
    tab_idx: usize,
    keys: Vec<Keystroke>,
    delay: Duration,
    next_key: usize,
    next_at: Instant,
}

/// A single tab: a PTY child process paired with its terminal emulator.
///
/// When `pty` is `None` the tab is dead — the child has exited but the tab
/// remains in the list so the user can relaunch it.
pub struct Tab {
    pub name: String,
    pub shortcut: char,
    pub command: Vec<String>,
    /// Working directory used whenever the tab's process is spawned.
    pub cwd: PathBuf,
    pub pty: Option<pty::Pty>,
    pub terminal: terminal::Terminal,
    pub proc_stat: Option<ProcStat>,
}

/// Manages multiple terminal tabs, only one of which is visible at a time.
pub struct Workspace {
    tabs: Vec<Tab>,
    active: usize,
    /// Per-tab, per-event match gates.
    event_gates: Vec<Vec<MatchGate>>,
    /// Delayed keystroke actions waiting to be sent.
    pending_actions: VecDeque<PendingAction>,
}

impl Workspace {
    /// Spawn every tab from the config. The first tab marked `default` becomes active.
    ///
    /// Tabs without a `cwd` start in the directory deskitty was launched from.
    pub fn spawn_all(tab_configs: &[TabConfig], rows: u16, cols: u16) -> anyhow::Result<Self> {
        let launch_dir = std::env::current_dir().context("resolving launch directory")?;
        let mut tabs = Vec::new();
        for tc in tab_configs {
            let cwd = tc.resolved_cwd().unwrap_or_else(|| launch_dir.clone());
            let pty = pty::Pty::spawn(rows, cols, &tc.command, &cwd).with_context(|| format!("spawning tab '{}'", tc.name))?;
            let proc_stat = pty.process_id().map(ProcStat::new);
            let terminal = terminal::Terminal::new(rows, cols);
            tabs.push(Tab { name: tc.name.clone(), shortcut: tc.shortcut, command: tc.command.clone(), cwd, pty: Some(pty), terminal, proc_stat });
        }

        let active = tab_configs.iter().position(|t| t.default).unwrap_or(0);
        let event_gates = (0..tabs.len()).map(|_| Vec::new()).collect();

        Ok(Self { tabs, active, event_gates, pending_actions: VecDeque::new() })
    }

    /// Index of the currently active tab.
    pub fn active_idx(&self) -> usize {
        self.active
    }

    /// Immutable reference to the active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Evaluate text-triggered rules for the active tab.
    ///
    /// Rules run top-to-bottom. A stable match fires once; repeated redraws at
    /// the same visible match count do nothing. Queued actions are executed in
    /// rule order by [`tick_actions`](Self::tick_actions).
    pub fn apply_events(&mut self, events: &[TextEvent], mode: ActionMode) -> io::Result<EventRun> {
        if mode != ActionMode::Active {
            self.pending_actions.clear();
        }
        if events.is_empty() {
            return Ok(EventRun::default());
        }
        self.sync_event_gates(events.len());

        let idx = self.active;
        let tab = &self.tabs[idx];
        if tab.pty.is_none() {
            return Ok(EventRun::default());
        }

        let now = Instant::now();
        let counts: Vec<usize> = events.iter().map(|event| event.match_count(tab.terminal.screen())).collect();
        let run = evaluate_events(events, &counts, &mut self.event_gates[idx], now);

        if mode == ActionMode::Active {
            for outcome in &run.outcomes {
                if let EventOutcome::Fired { event } = outcome
                    && let Some(action) = &events[*event].action
                {
                    self.enqueue_action(idx, action, now);
                }
            }
        }

        Ok(run)
    }

    /// Send the next due keystrokes from queued actions.
    pub fn tick_actions(&mut self, now: Instant, suppressed: bool) -> io::Result<()> {
        if suppressed {
            self.pending_actions.clear();
            return Ok(());
        }

        while let Some(mut action) = self.pending_actions.pop_front() {
            if action.next_key >= action.keys.len() {
                continue;
            }
            if now < action.next_at {
                self.pending_actions.push_front(action);
                break;
            }

            let keystroke = action.keys[action.next_key];
            action.next_key += 1;
            let finished = action.next_key >= action.keys.len();
            if !finished {
                action.next_at = now + action.delay;
            }

            let Some(tab) = self.tabs.get_mut(action.tab_idx) else { continue };
            let app_cursor = tab.terminal.application_cursor_keys();
            if let Some(ref mut pty) = tab.pty
                && let Some(bytes) = keystroke.to_bytes(app_cursor)
            {
                pty.write(&bytes)?;
            }

            if !finished {
                self.pending_actions.push_front(action);
            }
        }
        Ok(())
    }

    /// Drop every queued keystroke action.
    pub fn clear_pending_actions(&mut self) {
        self.pending_actions.clear();
    }

    fn enqueue_action(&mut self, tab_idx: usize, action: &EventAction, now: Instant) {
        if action.keystroke.is_empty() {
            return;
        }
        self.pending_actions.push_back(PendingAction { tab_idx, keys: action.keystroke.clone(), delay: action.delay, next_key: 0, next_at: now });
    }

    fn sync_event_gates(&mut self, event_count: usize) {
        self.event_gates.resize_with(self.tabs.len(), Vec::new);
        for gates in &mut self.event_gates {
            gates.resize_with(event_count, MatchGate::default);
        }
    }

    fn reset_event_gates(&mut self, idx: usize) {
        if let Some(gates) = self.event_gates.get_mut(idx) {
            for gate in gates {
                gate.reset();
            }
        }
    }

    /// Switch to the tab at the given index. No-op if out of bounds.
    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active = idx;
        }
    }

    /// Drain all PTYs (non-blocking reads) and feed bytes to their parsers.
    ///
    /// Returns `true` if any tab produced new output.
    pub fn poll_all(&mut self) -> io::Result<bool> {
        let mut dirty = false;
        for tab in &mut self.tabs {
            if let Some(ref mut pty) = tab.pty {
                let mut buf = [0u8; 4096];
                while let Some(n) = pty.try_read(&mut buf)? {
                    tab.terminal.process(&buf[..n]);
                    dirty = true;
                }
                for response in tab.terminal.drain_responses() {
                    pty.write(&response).ok();
                }
            }
        }
        Ok(dirty)
    }

    /// Resize every tab's terminal and PTY.
    pub fn resize_all(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        for tab in &mut self.tabs {
            tab.terminal.resize(rows, cols);
            if let Some(ref pty) = tab.pty {
                pty.resize(rows, cols)?;
            }
        }
        Ok(())
    }

    /// Collect `(shortcut, name)` pairs for the status bar.
    ///
    /// Dead tabs have ` (closed)` appended to their name.
    pub fn labels(&self) -> Vec<(char, String)> {
        self.tabs
            .iter()
            .map(|t| {
                let name = if t.pty.is_none() { format!("{} (closed)", t.name) } else { t.name.clone() };
                (t.shortcut, name)
            })
            .collect()
    }

    /// Return indices of all tabs whose child processes have terminated.
    pub fn exited_tabs(&mut self) -> io::Result<Vec<usize>> {
        let mut exited = Vec::new();
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if let Some(ref mut pty) = tab.pty
                && pty.try_wait()?.is_some()
            {
                exited.push(i);
            }
        }
        Ok(exited)
    }

    /// Mark a tab as closed (drop its PTY).
    pub fn close_tab(&mut self, idx: usize) {
        self.tabs[idx].pty = None;
        self.reset_event_gates(idx);
        self.pending_actions.retain(|action| action.tab_idx != idx);
    }

    /// Returns `true` if the tab at `idx` has no running PTY.
    pub fn is_dead(&self, idx: usize) -> bool {
        self.tabs[idx].pty.is_none()
    }

    /// Re-launch a previously closed tab in its original working directory.
    pub fn relaunch(&mut self, idx: usize, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.reset_event_gates(idx);
        self.pending_actions.retain(|action| action.tab_idx != idx);
        let tab = &mut self.tabs[idx];
        let pty = pty::Pty::spawn(rows, cols, &tab.command, &tab.cwd).with_context(|| format!("relaunching tab '{}'", tab.name))?;
        tab.pty = Some(pty);
        tab.proc_stat = tab.pty.as_ref().and_then(|p| p.process_id()).map(ProcStat::new);
        tab.terminal = terminal::Terminal::new(rows, cols);
        Ok(())
    }

    /// Returns `true` if every tab is dead.
    pub fn all_dead(&self) -> bool {
        self.tabs.iter().all(|t| t.pty.is_none())
    }

    /// Poll CPU jiffies for all non-active tabs.
    ///
    /// Returns `true` if any background tab was active within the last 500 ms.
    pub fn background_active(&mut self, _active_idx: usize) -> bool {
        let timeout = std::time::Duration::from_millis(500);
        let mut any_active = false;
        for tab in self.tabs.iter_mut() {
            if let Some(ref mut ps) = tab.proc_stat {
                if !ps.is_active(timeout) {
                    let _ = ps.poll();
                }
                if ps.is_active(timeout) {
                    any_active = true;
                }
            }
        }
        any_active
    }
}
