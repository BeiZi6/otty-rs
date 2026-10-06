<!-- source: https://docs.otty.sh/vt/esc/hts -->

# HTS — Set Tab Stop

|  |  |
| --- | --- |
| Sequence | ESC H |
| Bytes | 0x1B 0x48 |
| Mnemonic | HTS |
| Otty support | ✓ |

## Description

Sets a horizontal tab stop at the cursor's current column. Subsequent [HT](/vt/c0/ht) presses (`\t`) will advance to the next stop.

Tab stops are per-screen and reset by [RIS](/vt/esc/ris). To clear stops individually, use `CSI g` (clear at cursor) or `CSI 3 g` (clear all).

## Example
bash
```
printf '\e[20G\eH'      # move to column 20, set a tab stop there
printf 'a\tb\n'         # 'a', tab (now to col 20), 'b'
```

## Related

- [HT](/vt/c0/ht)
