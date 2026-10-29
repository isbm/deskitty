# Deskitty

**A terminal workspace manager for TUI applications.**

Deskitty runs several interactive terminal programs inside a single window and
lets you switch between them. Each tab owns a full pseudo-terminal with
256-color and 24-bit RGB emulation. The active tab fills the grid; the status
bar shows the available tabs.

The configuration is a YAML file. Tabs, working directories, screen automation,
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
- Optional embedded action sounds.
- Debug mode that logs observed screen attributes without sending keys.
- Optional borderless full-screen mode.

## Quick start

```sh
make dev
./target/debug/deskitty
```

Without a configuration file, deskitty starts a single `bash` tab.

To use a configuration file:

```sh
./target/debug/deskitty -c ~/.config/deskitty/config.yaml
```

The default path is `~/.config/deskitty/config.yaml`, so passing `-c` is only
needed for a different file.

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

- [Configuration](configuration.md) — tabs, working directories, desktop, menu.
- [Events and rules](events.md) — matching, ordering, guards, disappearance
  timers, sounds, delays.
- [Keystrokes](keystrokes.md) — complete key reference and modifiers.
- [Debug mode](debugging.md) — scanning screen attributes and tuning selectors.
- [Building](building.md) — development, tests, and documentation site.
