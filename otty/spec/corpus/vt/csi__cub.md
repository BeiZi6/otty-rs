<!-- source: https://docs.otty.sh/vt/csi/cub -->

# CUB — Cursor Back [​](#cub-—-cursor-back)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> D |
| Bytes | 0x1B 0x5B <n> 0x44 |
| Mnemonic | CUB |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor `<n>` columns to the left in the current row. Default `<n> = 1`. Clamps at column 1; no wrap.

## Example [​](#example)
bash
```
printf '\e[5D'      # back 5 columns
```

## Related [​](#related)

- [CUU](/vt/csi/cuu), [CUD](/vt/csi/cud), [CUF](/vt/csi/cuf), [CUP](/vt/csi/cup)
- [BS](/vt/c0/bs) — single-cell back (no scrolling, no SGR effect either way).
