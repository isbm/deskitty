use std::io;

use crate::config::TabConfig;
use crate::pty;
use crate::terminal;

/// A single tab: a PTY child process paired with its terminal emulator.
pub struct Tab {
    pub name: String,
    pub shortcut: char,
    pub pty: pty::Pty,
    pub terminal: terminal::Terminal,
}

/// Manages multiple terminal tabs, only one of which is visible at a time.
pub struct Workspace {
    tabs: Vec<Tab>,
    active: usize,
}

impl Workspace {
    /// Spawn every tab from the config. The first tab marked `default` becomes active.
    pub fn spawn_all(tab_configs: &[TabConfig], rows: u16, cols: u16) -> anyhow::Result<Self> {
        let mut tabs = Vec::new();
        for tc in tab_configs {
            let pty = pty::Pty::spawn(rows, cols, &tc.command)?;
            let terminal = terminal::Terminal::new(rows, cols);
            tabs.push(Tab { name: tc.name.clone(), shortcut: tc.shortcut, pty, terminal });
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
            let mut buf = [0u8; 4096];
            while let Some(n) = tab.pty.try_read(&mut buf)? {
                tab.terminal.process(&buf[..n]);
                dirty = true;
            }
        }
        Ok(dirty)
    }

    /// Resize every tab's terminal and PTY.
    pub fn resize_all(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        for tab in &mut self.tabs {
            tab.terminal.resize(rows, cols);
            tab.pty.resize(rows, cols)?;
        }
        Ok(())
    }

    /// Collect `(shortcut, name)` pairs for the status bar.
    pub fn labels(&self) -> Vec<(char, String)> {
        self.tabs.iter().map(|t| (t.shortcut, t.name.clone())).collect()
    }

    /// Returns `true` if any tab's child process has exited.
    pub fn any_exited(&mut self) -> io::Result<bool> {
        for tab in &mut self.tabs {
            if tab.pty.try_wait()?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
