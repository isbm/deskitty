# Deskitty

**A terminal workspace manager for TUI applications.**

Deskitty runs several interactive terminal programs inside a single window and
lets you switch between them. Each tab owns a full pseudo-terminal with
256-color and 24-bit RGB emulation. The active tab fills the grid; a status bar
shows the available tabs.

Configuration is a YAML file. Tabs, working directories, screen automation,
debug scanning, and the desktop frame are all controlled from it.

## Features

- Multiple tabs, each with its own PTY and terminal emulator.
- Full-screen terminal grid with resize support.
- Tab switching through a command menu.
- Configurable working directory per tab.
- Text-triggered automation with background-colour matching.
- Disappearance triggers with a configurable timeout.
- Ordered rules with IDs, guards, and skip targets.
- Ordered keystroke lists with delays.
- Optional repeating actions while a match remains visible.
- Optional embedded sounds per action.
- Startup switch to disable automatic actions.
- Debug trace console for matches, disappearance lifecycles, and fired actions.
- Optional borderless full-screen mode.

## Quick start

```sh
make dev
./target/debug/deskitty
```

Without a configuration file, deskitty starts a single `bash` tab. The default
config path is `~/.config/deskitty/config.yaml`; use `-c` for a different file.

```sh
./target/debug/deskitty -c path/to/config.yaml
```

## Controls

| Key | Action |
| --- | --- |
| `Ctrl+\` | Open or close the command menu |
| `Up` / `Down` | Move through menu items |
| `Enter` | Activate the selected item |
| Shortcut letter | Activate a tab or menu item directly |
| `Esc` | Close the menu |
| `q` | Quit from the command menu |

Keys typed while the menu is closed go directly to the active tab.

## Documentation

Full documentation lives in [`docs/`](docs/) and is generated as a static
website with MkDocs:

```sh
python3 -m pip install -r requirements-docs.txt
make docs
```

The generated site is written to `site/`. Preview it locally with:

```sh
make docs-serve
```

- [Configuration](docs/configuration.md)
- [Events and rules](docs/events.md)
- [Keystrokes](docs/keystrokes.md)
- [Debug mode](docs/debugging.md) — trace console and selector workflow
- [Building](docs/building.md)

## Building

```sh
make dev       # debug build
make release   # optimized release build
make check     # format and clippy
make test      # cargo-nextest
make clean     # remove build artifacts
```

## Status

Deskitty is in an early stage. The current version proves the core
architecture: PTY spawning, full terminal emulation, Ratatui compositing,
multi-tab management, configurable events, and a popup command menu.
