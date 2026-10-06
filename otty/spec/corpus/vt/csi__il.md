<!-- source: https://docs.otty.sh/vt/csi/il -->

# IL — Insert Lines

|  |  |
| --- | --- |
| Sequence | CSI <n> L |
| Bytes | 0x1B 0x5B <n> 0x4C |
| Mnemonic | IL |
| Otty support | ✓ |

## Description

Inserts `<n>` blank rows at the cursor, scrolling subsequent rows down within the active region. Rows scrolled past the bottom of the region are lost (they do **not** enter scrollback).

Default `<n> = 1`. Only effective inside the scrolling region (set by `DECSTBM`).

## Example
bash
```
printf '\e[5;1H'        # row 5, col 1
printf '\e[2L'          # insert 2 blank lines; rows 5..bot shift down 2
```

## Related

- [DL](/vt/csi/dl) — delete lines (inverse).
