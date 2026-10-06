<!-- source: https://docs.otty.sh/vt/csi/cud -->

# CUD — Cursor Down [​](#cud-—-cursor-down)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> B |
| Bytes | 0x1B 0x5B <n> 0x42 |
| Mnemonic | CUD |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor down `<n>` rows in the current column. Default `<n> = 1`. Does not scroll — clamps at the bottom of the active region.

## Example [​](#example)
bash
```
printf '\e[3B'      # move 3 rows down
```

## Related [​](#related)

- [CUU](/vt/csi/cuu), [CUF](/vt/csi/cuf), [CUB](/vt/csi/cub), [CUP](/vt/csi/cup)
