# Events and rules

Events watch the visible screen of the active tab and perform actions when
configured text appears or disappears. Rules are evaluated **top to bottom**
every tick.

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

## Repeat actions

Actions fire once per new stable match by default. Add `repeat` to keep firing
while the match remains visible:

```yaml
action:
  keystroke: [down]
  repeat: 0.5
```

- `repeat` is in seconds.
- The first repetition happens after the normal stable-match window.
- While the match is visible, the action fires again every `repeat` seconds.
- Repetition stops when the match disappears.
- If the match reappears, repetition starts again.
- Omit `repeat` for unique-appearance mode.
- `repeat` must be greater than zero.

`delay` and `repeat` serve different purposes:

- `delay` is the pause between keys inside one repetition.
- `repeat` is the pause between repetitions.

For example, this sends `right`, waits 100 ms, sends `enter`, then waits 500 ms
before starting the whole sequence again:

```yaml
action:
  keystroke:
    - right
    - enter
  delay: 0.1
  repeat: 0.5
```

If the keystroke block itself takes longer than `repeat`, the next repetition
waits for the block to finish rather than queuing up.

Repetition behavior:

- Skipped actions do not fire, but their repeat cadence continues.
- `Stop actions` and debug mode send nothing.
- Sounds repeat with the action.
- In debug mode, every would-be repetition logs its own `FIRE` line.

## On disappear

`on_disappear` fires an action after its input disappears for a timeout. This
is useful for detecting when a progress message such as `Thinking ...` has
finished:

```yaml
events:
  on_disappear:
    - id: "agent-finished"
      input:
        content: "Thinking ..."
        # background: ["#005f00"]   # optional, same as on_text
      timeout: 10                  # seconds
      action:
        sound: tada
```

- `on_disappear` accepts one rule or a list.
- `input` uses the same `content` and `background` matching as `on_text`.
- `timeout` is required and is in seconds. It must be greater than zero.
- `action` uses the same keystrokes, `delay`, and `sound` fields as `on_text`.
- `repeat` is not allowed on an `on_disappear` action.
- `id` is optional and is used as a label in debug output.
- `on_disappear` rules do not take part in `skip` guards.

Lifecycle:

1. The rule waits until its input becomes visible.
2. Once visible, the rule is armed.
3. When the input disappears, the timeout starts.
4. If the input reappears before the timeout, the timer is cancelled and the
   rule stays armed.
5. If the input remains absent for the timeout, the action fires once and the
   rule waits for the input to reappear.
6. A later disappearance starts a new timeout and can fire again.

Consequences:

- Input that was never visible never fires.
- A brief flicker shorter than `timeout` does not fire.
- Continued absence after firing does not fire repeatedly.
- Reappearance re-arms the rule.
- `Stop actions` and debug mode suppress the action, but the timeout still
  consumes the firing; it is not replayed after resuming.

Like `on_text`, disappearance rules only track the active tab. Their state is
reset when you switch back to a tab, so time spent on other tabs is not
counted; the rule arms again once its input is visible. When both rule types
fire in the same tick, `on_text` actions are queued first, followed by
`on_disappear` actions.

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
