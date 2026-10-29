# Keystrokes

`action.keystroke` accepts one key or an ordered list:

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

Keys execute in list order. Use [`delay`](events.md#delays) to pause between
consecutive keys.

## Case sensitivity

Named keys and modifiers are case-insensitive:

```yaml
keystroke: [enter]
keystroke: [ENTER]
keystroke: [Enter]
```

Single-character keys preserve their case:

- `a` sends lowercase `a`
- `A` sends uppercase `A`

## Named keys

| Key | Aliases |
| --- | --- |
| `enter` | `return` |
| `esc` | `escape` |
| `tab` | |
| `backspace` | |
| `delete` | `del` |
| `insert` | `ins` |
| `up` | `arrowup` |
| `down` | `arrowdown` |
| `left` | `arrowleft` |
| `right` | `arrowright` |
| `home` | |
| `end` | |
| `pageup` | `pgup` |
| `pagedown` | `pgdn` |
| `space` | |
| `f1` … `f12` | |

## Modifiers

Combine a modifier with a key using `+`:

```yaml
action:
  keystroke:
    - ctrl+c
    - alt+x
    - shift+tab
```

Supported modifier names:

| Modifier | Aliases |
| --- | --- |
| `ctrl` | `control` |
| `alt` | `option` |
| `shift` | |

### Control keys

`ctrl+letter` is supported for `a` through `z`:

```yaml
keystroke: [ctrl+c, ctrl+d, ctrl+z]
```

Other control combinations, such as `ctrl+1` or `ctrl+space`, are rejected as
unsupported keystrokes.

### Shift

`shift+letter` does not automatically uppercase the character. Use the literal
uppercase character instead:

```yaml
keystroke: [A]
```

`shift+tab` sends the backtab sequence (`ESC [ Z`).

For other named keys, modifiers may encode as the unmodified key. Prefer plain
named keys unless the target program documents a specific combination.

## Single characters

Any single character is accepted:

```yaml
action:
  keystroke:
    - y
    - n
    - "1"
    - "?"
    - "/"
```

To send a literal plus sign, use `"+"`.

## Unsupported keys

The following are rejected when the configuration is loaded:

- media and browser keys
- keypad-specific keys
- modifier combinations that cannot be encoded
- multi-character strings that are not named keys or modifier combinations

Unknown keystrokes are reported as configuration errors before the terminal UI
starts.

## Debug output

In [debug mode](debugging.md), fired actions display their keys using canonical
names:

```text
FIRE allow-once RIGHT+ENTER
```

If the action also plays a sound:

```text
FIRE allow-once RIGHT+ENTER sound=accept
```
