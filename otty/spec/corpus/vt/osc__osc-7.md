<!-- source: https://docs.otty.sh/vt/osc/osc-7 -->

# OSC 7 — Current Working Directory

|  |  |
| --- | --- |
| Sequence | OSC 7 ; file://<host>/<path> ST |
| Otty support | ✓ |

## Description

Reports the pane's current working directory. The terminal stores it per-pane and uses it for:

- Resolving relative paths in [Files and Links](/user-interface/files-and-links).
- The cwd anchor of newly-spawned panes (split / new tab via shortcut).
- The cwd shown in the [Window, Tab and Split](/user-interface/window-tab-split) commands menu.

The payload is a `file://` URI. The `<host>` part should be the machine hostname; over SSH it lets the terminal differentiate local vs remote paths. `<path>` must be percent-encoded.

## Example
bash
```
# emitted by the shell at each prompt
printf '\e]7;file://%s%s\e\\' "$HOSTNAME" "$(pwd | sed 's| |%20|g')"

# zsh precmd snippet (Otty's shell integration installs this for you):
precmd() { print -Pn "\e]7;file://%m%d\e\\"; }
```

Try it

`otty features try osc7` — emits the OSC 7 sequence for the current directory once. Subsequent new tabs / splits inherit it.

## Related

- [Shell Integration](/terminal-features/shell-integration) — installs OSC 7 emission automatically.
- [OSC 133](/vt/osc/osc-133) — prompt marks, the other half of integration.
