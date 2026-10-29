# Events and rules

Events watch the visible screen of the active tab and perform actions when
configured text appears. Rules are evaluated **top to bottom** every tick.

## Basic rule

```yaml
events:
  on_text:
    - id: "allow-once"
      input:
        content: "Allow once"
        background: ["#005f00"]
      action:
        keystroke: [enter]
```

- `on_text` accepts one rule or a list.
- `content` is a literal, case-sensitive substring.
- `background` is optional.
- `action` is optional; a rule without one is a pure guard.

## Input matching

```yaml
input:
  content: "Allow once"
  background: ["#005f00"]
```

`content` matches across wrapped rows on the visible screen. Explicit newlines
inside the string are not supported.

`background` is an optional allowlist. When present, every non-space character
in the match must have one of the listed backgrounds. When omitted, any
background matches.

### Colors

`background` entries may be:

- ANSI names: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`,
  `white`, `gray`
- bright aliases such as `bright_green`
- `"!default"` for the terminal default background
- `"!black"` and other tagged names
- a numeric ANSI index, e.g. `22`
- an RGB value, e.g. `"#005f00"`

Named colors match both their normal and bright ANSI variants. Indexed colors
are normalised through the xterm 256-color palette, so a configured
`"#005f00"` matches both truecolor output and the equivalent indexed color.

Inverse-video cells are matched using their effective background: the
foreground color becomes the background for matching purposes.

## Rule order

`on_text` is an ordered list. Rules are evaluated from top to bottom.

```yaml
events:
  on_text:
    - id: "first"
      input:
        content: "one"
      action:
        keystroke: [right]

    - id: "second"
      input:
        content: "two"
      action:
        keystroke: [enter]
```

When several rules fire in the same tick, their actions are queued in
configuration order and executed in that order.

Each rule also has a match gate: it fires once when the number of visible
matches rises above the number already handled. Repeated redraws or scrolling
at the same count do nothing. This keeps a prompt embedded in a scrolling
transcript from firing repeatedly.

## IDs and guards

Every rule can have an `id`. IDs must be unique. A guard rule can skip the
actions of rules below it:

```yaml
events:
  on_text:
    - id: "deny-script"
      input:
        content: "python3"
      skip: ["allow-once"]
      action:
        keystroke: [esc]

    - id: "allow-once"
      input:
        content: "Allow once"
        background: ["#005f00"]
      action:
        keystroke: [enter]
```

If both inputs match, `deny-script` sends `Esc` and suppresses `allow-once`.

### Skip semantics

- `skip` is declared on the guard rule, not on the action.
- Targets must be defined **below** the guard.
- Skip applies while the guard's input is visible.
- A skipped rule still updates and consumes its match gate, so it does not
  fire later when the guard disappears.
- Skip declarations are based on the guard's input match. They apply even if
  the guard's own keystroke action is skipped by an earlier guard.
- A guard can both send keystrokes and skip targets.
- A pure guard has no `action`:

```yaml
- id: "deny-script"
  input:
    content: "python3"
  skip: ["allow-once"]
```

### Validation

These are configuration errors:

- duplicate rule IDs,
- empty IDs,
- empty skip targets,
- unknown skip target IDs,
- skip targets that appear above the guard.

Errors are reported before the terminal UI starts.

## Keystrokes

`action.keystroke` accepts a single key or an ordered list:

```yaml
action:
  keystroke: enter
```

```yaml
action:
  keystroke:
    - right
    - enter
```

Keys execute in list order. See [Keystrokes](keystrokes.md) for the complete
reference, including named keys, aliases, modifiers, and single characters.

## Delays

`delay` is the pause between consecutive keystrokes, in seconds:

```yaml
action:
  keystroke:
    - right
    - enter
  delay: 0.1
```

`0.1` means 100 milliseconds. A single keystroke has no internal delay.

The delay scheduler is non-blocking. Rendering and input continue while keys
are queued. Multiple firing rules execute their action blocks in configuration
order.

Stopping actions, closing the tab, or entering debug mode clears the pending
action queue.

## Sounds

`action.sound` optionally plays an embedded WAV when the action fires:

```yaml
action:
  keystroke: [enter]
  sound: accept
```

Available sounds are:

- `accept`
- `tada`
- `wxp`

A sound-only action is also valid:

```yaml
action:
  sound: tada
```

Semantics:

- The sound plays when the action fires, before its keystrokes.
- Skipped actions play no sound.
- `Stop actions` and debug mode play no sound.
- Unknown sound names are configuration errors.
- The WAV files are embedded in the binary at build time. Keep them under
  `sounds/` in the repository.

Playback uses the first available system player:

1. `paplay`
2. `pw-play`
3. `aplay`
4. `ffplay`

If sounds are configured in working mode and none of these players is
available, deskitty shows a gradient-bordered startup notice with a
`Fine  :-(` button. The notice is dismissed with `Enter`, `Esc`, or `Space`.
Actions are paused while the notice is open. After dismissal, actions and
normal typing work, but sounds remain disabled.

The notice does not appear when:

- no action uses `sound:`,
- debug mode is active,
- a supported player is available and its sound files were prepared successfully.

## Complete example

```yaml
events:
  on_text:
    - id: "deny-script"
      input:
        content: "python3"
      skip: ["allow-once"]
      action:
        keystroke: [esc]

    - id: "allow-once"
      input:
        content: "Allow once"
        background: ["#005f00"]
      action:
        keystroke:
          - right
          - enter
        delay: 0.1
        sound: accept
```

## Debugging selectors

Use [Debug mode](debugging.md) to inspect the actual foreground, background,
and text attributes before trusting a selector.
