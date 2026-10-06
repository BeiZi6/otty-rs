<!-- source: https://docs.otty.sh/vt/c0/ht -->

# HT — Horizontal Tab [​](#ht-—-horizontal-tab)
****``********
|  |  |
| --- | --- |
| Byte | 0x09 |
| Mnemonic | HT |
| Otty support | ✓ |

## Description [​](#description)

Advances the cursor to the next tab stop. Default tab stops are every 8 columns. Tab stops are set with [HTS](/vt/esc/hts) (`ESC H`) and cleared with `CSI g`.

If the cursor is past the last tab stop on the line, HT moves it to the right margin without wrapping.

## Example [​](#example)
bash
```
printf 'col1\tcol2\tcol3\n'
```

## Related [​](#related)

- [HTS](/vt/esc/hts) — set a tab stop at the current column.
- `CSI g` — clear tab stops (omitted from the index here; rarely used).
