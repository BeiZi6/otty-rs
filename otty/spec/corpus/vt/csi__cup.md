<!-- source: https://docs.otty.sh/vt/csi/cup -->

# CUP — Cursor Position

|  |  |
| --- | --- |
| Sequence | CSI <row> ; <col> H (or f) |
| Bytes | 0x1B 0x5B <row> 0x3B <col> 0x48 |
| Category | CSI |
| Mnemonic | CUP |
| Otty support | ✓ Implemented |

## Description

Moves the cursor to the given absolute position. Row and column are **1-indexed** — `CSI 1 ; 1 H` is the top-left cell.

Defaults: omitted parameters are treated as `1`. So `CSI H` moves to home (`1, 1`).

If a scrolling region is active (`DECSTBM`), positions are bounded inside that region when `DECOM` (origin mode) is set; otherwise they're absolute to the viewport.

## Aliases

- `CSI <r>;<c> H` — canonical
- `CSI <r>;<c> f` — HVP (Horizontal and Vertical Position). Identical behaviour in modern terminals.

## Example
bash
```
# Move to row 5, column 10
printf '\e[5;10H'

# Move to home (top-left)
printf '\e[H'

# Repaint a status bar at the bottom of a 24-row screen
printf '\e[24;1H[ ready ]'
```

## Related

- `CSI <n> A/B/C/D` — relative cursor movement (CUU/CUD/CUF/CUB)
- `CSI <n> G` — Cursor Horizontal Absolute (column only)
- `CSI <n> d` — Vertical Position Absolute (row only)
- `ESC 7` / `ESC 8` — save / restore cursor (full state, not just position)

## Notes

- Out-of-range positions clamp to the viewport edge.
- The cursor reports its position via [`CSI 6 n` (DSR)](/vt/csi/dsr) → `CSI <row>;<col> R`.
