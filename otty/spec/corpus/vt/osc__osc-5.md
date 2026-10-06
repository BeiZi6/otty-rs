<!-- source: https://docs.otty.sh/vt/osc/osc-5 -->

# OSC 5 — "Special" Color Set

|  |  |
| --- | --- |
| Sequence | OSC 5 ; <slot> ; rgb:R/G/B ST (set) · OSC 5 ; <slot> ; ? ST (query) |
| Otty support | ✓ parsed and stored · underline (slot 1) consulted as fallback when no SGR 58 is active on the cell |

## Description

xterm-style "special color" slots — colors that decorate one of the SGR attributes globally rather than per-cell. Less widely used than [OSC 10/11/12](/vt/osc/osc-10-19) but a small number of programs (kitty themes, the [base16 schemes](https://github.com/base16-project), some debuggers' integrated REPLs) emit them.
``
| <slot> | Meaning |
| --- | --- |
| 0 | Bold-text color |
| 1 | Underline color (default; SGR 58 still overrides per-cell) |
| 2 | Blink color |
| 3 | Reverse-video color |
| 4 | Italic-text color |

Per-cell SGR 58 (`\e[58:2::R:G:B`) wins over slot 1 — that's xterm's documented precedence. Otty treats the OSC 5 underline color as the *default* underline tint applied when the cell has no explicit SGR 58.

## Reset

The companion sequence is [OSC 105](/vt/osc/osc-104-119#osc-105---reset-special-colors): `OSC 105 ST` resets all five slots; `OSC 105 ; 1 ; 4 ST` resets just slots 1 and 4.

## Example
bash
```
# Make undercurl underlines render in red even when no SGR 58 is set
printf '\e]5;1;#ff5555\e\\'
printf '\e[4:3mtypo without SGR 58\e[0m\n'

# Restore
printf '\e]105;1\e\\'
```

## Notes

- Bold / blink / reverse / italic slots are stored but Otty's renderer doesn't currently consult them — bold uses its own `bold-color` config + `bold-is-bright` toggle, italics use the per-cell SGR fg, and Otty doesn't blink. The values are queryable, kept in the per-session overrides table, and will be wired through when a follow-up feature needs them.

## Related

- [OSC 10/11/12/17/19 — main color slots](/vt/osc/osc-10-19)
- [OSC 105 — reset specials](/vt/osc/osc-104-119#osc-105---reset-special-colors)
- [SGR — per-cell colors](/vt/csi/sgr)
