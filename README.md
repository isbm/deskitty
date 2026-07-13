# Deskty

**A terminal workspace manager for TUI applications.**

Deskty is a compositor that runs multiple interactive terminal programs inside a single window and lets you switch between them. Think of it as a minimal tab-based wrapper: each tab holds a full terminal session with 256-color and 24-bit RGB emulation, and you toggle between them with a popup menu.

It is built for a world where developers keep several terminal-based tools open at once — a shell here, OpenCode there, a process monitor in the corner — and want to stay inside one terminal window without giving up the feel of a real terminal.

## How it feels

Launch deskitty with a config file that names your tabs:

```sh
deskitty -c demo/agents/config.yaml
```

Deskty enters fullscreen mode. Each tab runs its own program inside a pseudo-terminal. The active tab renders at full size with complete terminal emulation. A status bar sits at the bottom showing all open tabs and the current one highlighted.

Press `Ctrl+\` anywhere to bring up the command menu. Switch tabs by selecting one with the arrow keys or pressing its shortcut letter. The menu dismisses with `Esc`, and `q` quits the whole session.

```
┌──────────────────────────────────────────────┐
│                                              │
│             opencode (active tab)            │
│              full terminal grid              │
│                                              │
├──────────────────────────────────────────────┤
│ o:OpenCode | *s:Shell* | Ctrl+\ Menu         │
└──────────────────────────────────────────────┘
```

Keys typed while the menu is hidden flow directly to the active tab. The child process sees a real terminal — it can move the cursor, set colors, use the alternate screen, and receive resize events.

## Config

Tabs are declared in a YAML file:

```yaml
tabs:
  - name: "OpenCode"
    shortcut: "o"
    command: ["opencode"]
    default: true
  - name: "Shell"
    shortcut: "s"
    command: ["bash"]
```

The config is optional. Without it, deskitty starts a single `bash` tab.

## How it is built

Deskty is written in Rust. The rendering stack uses Ratatui for layout and compositing, Crossterm for raw terminal I/O, a `vt100`-backed cell grid for ANSI parsing, and `portable-pty` for pseudo-terminal management.

The entire frame — terminal grid, status bar, and popup menu — is assembled by Ratatui on each tick. Child output flows from the PTY into the ANSI parser, which pushes styled cells onto the grid. Key presses flow the other direction, through Crossterm into the active PTY.

## Building

```sh
make dev       # debug build
make release   # optimized release build
make check     # format + clippy
make clean     # remove build artifacts
```

Just `make` prints the available targets.

## Status

Deskty is in an early stage. The current version proves the core architecture: PTY spawning, full terminal emulation, Ratatui compositing, multi-tab management, and a popup command menu. More features are planned but not yet built.
