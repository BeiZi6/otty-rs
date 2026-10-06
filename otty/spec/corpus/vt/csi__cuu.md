<!-- source: https://docs.otty.sh/vt/csi/cuu -->

# CUU — Cursor Up [​](#cuu-—-cursor-up)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> A |
| Bytes | 0x1B 0x5B <n> 0x41 |
| Mnemonic | CUU |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor up `<n>` rows in the current column. Default `<n> = 1`. The cursor does not move past the top of the active region (no scroll).

## Example [​](#example)
bash
```
printf '\e[3A'      # move 3 rows up
printf '\e[A'       # move 1 row up
```

## Related [​](#related)

- [CUD](/vt/csi/cud), [CUF](/vt/csi/cuf), [CUB](/vt/csi/cub), [CUP](/vt/csi/cup)
