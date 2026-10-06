<!-- source: https://docs.otty.sh/vt/csi/cuf -->

# CUF — Cursor Forward [​](#cuf-—-cursor-forward)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> C |
| Bytes | 0x1B 0x5B <n> 0x43 |
| Mnemonic | CUF |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor `<n>` columns to the right in the current row. Default `<n> = 1`. Clamps at the right margin; no wrap.

## Example [​](#example)
bash
```
printf '\e[10C'     # advance 10 columns
```

## Related [​](#related)

- [CUU](/vt/csi/cuu), [CUD](/vt/csi/cud), [CUB](/vt/csi/cub), [CUP](/vt/csi/cup)
