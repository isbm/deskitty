use std::io;
use std::path::PathBuf;

use anyhow::Context;

use crate::config::TabConfig;
use crate::proc::ProcStat;
use crate::pty;
use crate::terminal;

/// A single tab: a PTY child process paired with its terminal emulator.
///
/// When `pty` is `None` the tab is dead — the child has exited but the tab
/// remains in the list so the user can relaunch it.
pub struct Tab {
    pub name: String,
    pub shortcut: char,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub pty: Option<pty::Pty>,
    pub terminal: terminal::Terminal,
    pub proc_stat: Option<ProcStat>,
}

/// Manages multiple terminal tabs, only one of which is visible at a time.
pub struct Workspace {
    tabs: Vec<Tab>,
    active: usize,
}

impl Workspace {
    /// Spawn every tab from the config. The first tab marked `default` becomes active.
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

        Ok(Self { tabs, active })
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
    }

    /// Returns `true` if the tab at `idx` has no running PTY.
    pub fn is_dead(&self, idx: usize) -> bool {
        self.tabs[idx].pty.is_none()
    }

    /// Re-launch a previously closed tab.
    pub fn relaunch(&mut self, idx: usize, rows: u16, cols: u16) -> anyhow::Result<()> {
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
