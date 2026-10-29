# Debug mode

Debug mode scans configured rules and reports what the terminal actually
contains, without sending any keystrokes.

```yaml
debug: true
```

The flag is read at startup. To switch back to working mode, set
`debug: false` and restart.

## Overlay

When debug mode is active and at least one rule exists, a floating panel is
drawn in the top-right corner:

- width: 50 columns
- height: 5 rows total
- 1 row from the top
- 4 columns from the right

The panel shows a title, then the newest log lines:

```text
SCAN "Allow once" [1] fire=1 skip=0
```

- `[1]` is the number of visible content matches.
- `fire` is the number of actions that would have fired since debug started.
- `skip` is the number of actions suppressed by guards since debug started.

## Match samples

Every visible match is sampled, including matches that do not satisfy the
configured background condition.

Each distinct sample appends two lines:

```yaml
background: ["#005f00"]
fg=#d7ffd7 bg=#005f00 B0D0I0U0R0 ok
```

The first line is valid YAML and can be copied into the rule input.

The second line reports:

- `fg` — foreground color
- `bg` — background color
- `B` — bold
- `D` — dim
- `I` — italic
- `U` — underline
- `R` — inverse
- `0` / `1` — attribute off / on

The final word compares the sample with the rule's configured background:

| Value | Meaning |
| --- | --- |
| `ok` | The configured background matches this sample. |
| `no` | The configured background does not match this sample. |
| `any` | The rule has no background condition. |

Colors are normalised to `#rrggbb` or `"!default"`, so the copied line matches
both truecolor and indexed output. The copy-paste line uses the effective
background, so inverse-video samples report the color the matcher actually
sees.

Samples with identical attributes are not appended repeatedly. Scrolling or
redrawing the same content does not flood the log.

## Fire and skip lines

When a rule's action would fire:

```text
FIRE allow-once RIGHT+ENTER sound=accept
```

If the action has no sound, the `sound=` suffix is omitted.

When an earlier guard suppresses an action:

```text
SKIP allow-once <- deny-script
```

No keystrokes are sent in debug mode, and action sounds are not played. These
lines show what working mode would do.

## Selector workflow

1. Set `debug: true`.
2. Run deskitty and trigger the screen state you want to match.
3. Find the sample that represents the desired row.
4. Copy its `background:` line into the rule input.
5. Set `debug: false` and restart.

For example, if the desired row reports:

```yaml
background: ["#005f00"]
```

use:

```yaml
input:
  content: "Allow once"
  background: ["#005f00"]
```

Other occurrences of the same text with different backgrounds will no longer
match.

## Menu behavior

Debug mode already suppresses all automatic actions. The command menu keeps the
actions toggle visible but disabled:

```text
Actions paused      debug
```

`Hide desktop` and `Show desktop` remain available.
