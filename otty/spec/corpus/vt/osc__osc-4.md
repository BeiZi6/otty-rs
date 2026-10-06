<!-- source: https://docs.otty.sh/vt/osc/osc-4 -->

# OSC 4 — Palette Set / Query

|  |  |
| --- | --- |
| Sequence | OSC 4 ; <index> ; rgb:R/G/B ST (set) · OSC 4 ; <index> ; ? ST (query) |
| Otty support | ✓ (full 0–255 range) |

## Description

Sets or queries one entry of the terminal's 256-color palette — the same indices that `SGR 38;5;<n>` / `SGR 48;5;<n>` reference. Used by tools that ship their own color schemes (`base16`, `dotfiles` color managers) without touching the terminal config.

| Range | Meaning |
| --- | --- |
| 0–15 | ANSI 16 (the standard palette colors) |
| 16–231 | 6×6×6 RGB cube |
| 232–255 | 24-step grayscale ramp |

## Color spec accepted

Same family as [OSC 10/11/12](/vt/osc/osc-10-19): `rgb:RR/GG/BB`, `rgb:R/G/B`, `rgb:RRRR/GGGG/BBBB`, `#RGB`, `#RRGGBB`, `#RRRGGGBBB`, `#RRRRGGGGBBBB`.

## Examples
bash
```
# Make ANSI 1 (red) a bright orange-red
printf '\e]4;1;#ff5533\e\\'

# base16-style — apply a whole 16-color scheme in one pass
for i in 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
  printf '\e]4;%d;%s\e\\' "$i" "${TOKYONIGHT_PALETTE[$i]}"
done

# Query ANSI 4 (blue) — reply lands on stdin
printf '\e]4;4;?\e\\'
```

## Reset

[OSC 104](/vt/osc/osc-104-119) — `printf '\e]104;1\e\\'` resets index 1; bare `printf '\e]104\e\\'` resets all 256.

## Related

- [OSC 10/11/12/17/19](/vt/osc/osc-10-19) — named slots beyond the palette.
- [OSC 104](/vt/osc/osc-104-119) — palette reset companion.
- [CSI #P/Q/R — XTPUSHCOLORS](/vt/csi/xt-colors)
- [Themes](/customization/themes)
