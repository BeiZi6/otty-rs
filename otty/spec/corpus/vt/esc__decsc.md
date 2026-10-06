<!-- source: https://docs.otty.sh/vt/esc/decsc -->

# DECSC — Save Cursor

|  |  |
| --- | --- |
| Sequence | ESC 7 |
| Bytes | 0x1B 0x37 |
| Mnemonic | DECSC |
| Otty support | ✓ |

## Description

Saves the cursor state for later restoration with [DECRC](/vt/esc/decrc). The saved state includes:

- Cursor row and column
- Active SGR attributes
- Character set designation (G0 / G1 selection)
- Origin mode (DECOM) state
- Auto-wrap (DECAWM) state

There is one save slot per screen — calling DECSC again overwrites the previous save. Save/restore is per-screen, so switching between main / alternate screen does not affect the other's saved state.

## Example
bash
```
printf '\e[5;10H\e7'      # move to (5,10), save
printf '\e[20;1H\e[1mbottom\e[m'  # do something elsewhere
printf '\e8'              # restore to (5,10) with original SGR
```

## Related

- [DECRC](/vt/esc/decrc) — restore.
- [CSI s / CSI u](/vt/csi/) — ANSI.SYS save/restore (alternative).
