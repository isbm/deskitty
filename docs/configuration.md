# Configuration

Deskitty reads a YAML configuration file. The default path is:

```text
~/.config/deskitty/config.yaml
```

Use `-c` to load a different file:

```sh
deskitty -c path/to/config.yaml
```

If the file does not exist, deskitty starts a single `bash` tab. A malformed
configuration is reported as an error at startup.

## Full example

```yaml
actions: true
show_desktop: true
debug: false

tabs:
  - name: "Editor"
    shortcut: "e"
    command: ["editor"]
    default: true

  - name: "Shell"
    shortcut: "s"
    command: ["bash"]
    cwd: "~/work/my-project"
```

## Tabs

Each tab is one program running inside its own pseudo-terminal.

| Field | Required | Description |
| --- | --- | --- |
| `name` | yes | Label shown in the menu and status bar. |
| `shortcut` | yes | Single character used to jump to the tab from the menu. |
| `command` | yes | Program and arguments as a YAML list. |
| `cwd` | no | Working directory for the program. |
| `default` | no | Starts this tab when deskitty launches. |

The first tab marked `default: true` becomes active. If no tab is marked,
the first tab is used.

`tabs` is an ordered list. Menu and status order follow the configuration
order.

### Working directory

If `cwd` is omitted, the tab starts in the directory where deskitty was
launched.

`cwd` accepts:

- an absolute path,
- a path starting with `~` or `~/` (expanded from `$HOME`),
- a path relative to deskitty's launch directory.

The directory must exist. An invalid directory is reported when the tab is
spawned.

## Actions

Automatic event actions are enabled by default:

```yaml
actions: true
```

Set it to `false` to start with actions paused:

```yaml
actions: false
```

While actions are paused, match gates continue to update, so resuming does not
replay prompts that appeared during the pause. Keystrokes and sounds are not
sent or played.

The setting only controls the starting state. It can be toggled at runtime from
the command menu.

## Desktop frame

The colored frame and status bar are shown by default:

```yaml
show_desktop: true
```

Set it to `false` to run without the frame or status bar:

```yaml
show_desktop: false
```

In that mode the terminal grid uses the entire window, and the PTYs are
resized to the full terminal dimensions. `Ctrl+\` is handled by deskitty
itself, so the command menu still opens while the desktop is hidden.

The setting only controls the starting state. It can be toggled at runtime
from the command menu.

## Command menu

Press `Ctrl+\` to open the menu. It contains, in order:

1. Tabs, in configuration order.
2. `Stop actions` / `Resume actions`.
3. `Hide desktop` / `Show desktop`.
4. `Quit`.

### Actions toggle

`Stop actions` pauses automatic event keystrokes and sounds. Normal typing is
unaffected. Match gates continue to update while paused, so resuming does not
replay prompts that appeared during the pause.

The toggle always starts in the state configured by `actions`.

### Desktop toggle

`Hide desktop` removes the frame and status bar and expands the terminal to the
full window. `Show desktop` restores them and resizes the terminal back to the
inset grid.

Both toggles take effect immediately without restarting.

## Debug mode

Set `debug: true` to show the trace console:

```yaml
debug: true
```

Debug mode only adds tracing. It does not change whether actions execute. To
trace without sending keystrokes or playing sounds, also set `actions: false`:

```yaml
actions: false
debug: true
```

See [Debug mode](debugging.md) for the console and workflow.

## Event rules

Text-triggered rules live under `events.on_text`. See
[Events and rules](events.md) for the full schema.
