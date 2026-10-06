<!-- source: https://docs.otty.sh/vt/osc/osc-10-19 -->

# OSC 10 / 11 / 12 / 17 / 19 — Dynamic Color Set / Query

|  |  |
| --- | --- |
| Sequence | OSC <code> ; rgb:R/G/B ST (set) · OSC <code> ; ? ST (query) |
| Otty support | ✓ 10 / 11 / 12 · ✓ 17 / 19 · OSC 13 / 14 parsed, not applied on macOS · OSC 15 / 16 / 18 parsed, not rendered (Tek 4014 is a dead protocol) |

## Description

Lets a program set or query one of the terminal's "named" colors at runtime — distinct from the 256-color palette ([OSC 4](/vt/osc/osc-4)) and from per-cell SGR colors.

| Code | Slot | Otty effect |
| --- | --- | --- |
| 10 | Text foreground | applied to default-fg cells |
| 11 | Text background | applied to default-bg cells + window |
| 12 | Cursor color | overrides cursor-color config |
| 13 | Mouse pointer fg | stored, no visible effect on macOS |
| 14 | Mouse pointer bg | stored, no visible effect on macOS |
| 15 | Tektronix foreground | stored, not rendered |
| 16 | Tektronix cursor | stored, not rendered |
| 17 | Highlight (selection) background | overrides selection-background |
| 18 | Tektronix background | stored, not rendered |
| 19 | Highlight (selection) foreground | overrides selection-foreground |

Query form (`OSC <code> ; ? ST`) replies in the canonical xterm format `OSC <code> ; rgb:HHHH/HHHH/HHHH ST` (16-bit per channel, replicated from 8 bits).

## Accepted color formats

- `rgb:RR/GG/BB` — 8-bit hex (xterm shorthand)
- `rgb:R/G/B` — 4-bit hex shorthand (`f/0/0` → `0xff 0x00 0x00`)
- `rgb:RRRR/GGGG/BBBB` — 16-bit canonical form (Otty truncates to high byte)
- `#RGB`, `#RRGGBB`, `#RRRGGGBBB`, `#RRRRGGGGBBBB`

## Reset pages

The reset companion sequences live at [OSC 104 / 110-119](/vt/osc/osc-104-119).

## Examples
bash
```
# Highlight the selection with a warm gold (kitty's "yellow on bright bg" style)
printf '\e]17;#665522\e\\'
printf '\e]19;#ffffe0\e\\'

# Switch the cursor to red while a debugger is attached, then restore
printf '\e]12;#ff5555\e\\'
# … run debugger …
printf '\e]112\e\\'   # OSC 112 — reset cursor color

# Query the current fg / bg
printf '\e]10;?\e\\\e]11;?\e\\'
# The reply lands on the shell's stdin — capture with:
#   printf '\e]10;?\e\\'; read -d $'\a' reply; echo "$reply"

# Try every code at once interactively:
otty features try change-color
```

## Notes

- OSC 10/11/12 affect *every* default-color cell on the next frame; pre-existing cells already painted with explicit SGR colors keep their color.
- OSC 17/19 only affect the selection overlay — they do not change the SGR 48 / 38 cell colors.
- OSC 13/14 (mouse pointer colors) are recorded but not applied — macOS has no way to recolor the mouse pointer, so the values are inert there.
- Query replies travel back through the PTY as if the user typed them — they land on the shell's stdin. Use `read` (bash/zsh) or `process_substitution` (fish) to capture them.

## Related

- [OSC 4 — palette set / query](/vt/osc/osc-4)
- [OSC 104 / 110-119 — reset](/vt/osc/osc-104-119)
- [CSI #P/Q/R — XTPUSHCOLORS](/vt/csi/xt-colors)
- [OSC 5 — special colors](/vt/osc/osc-5)
- [SGR — per-cell colors](/vt/csi/sgr)
