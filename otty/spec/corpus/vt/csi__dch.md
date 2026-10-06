<!-- source: https://docs.otty.sh/vt/csi/dch -->

# DCH — Delete Characters [​](#dch-—-delete-characters)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> P |
| Bytes | 0x1B 0x5B <n> 0x50 |
| Mnemonic | DCH |
| Otty support | ✓ |

## Description [​](#description)

Deletes `<n>` cells starting at the cursor, shifting remaining cells in the row to the left. Cells on the right are filled with blanks.

Default `<n> = 1`. Cursor position is unchanged.

## Example [​](#example)
bash
```
printf 'abcdef\e[H'     # write text, move home
printf '\e[2P'          # delete 2 chars at col 1
# row now reads "cdef  "
```

## Related [​](#related)

- [ICH](/vt/csi/ich)
- [EL](/vt/csi/el) — erase rather than delete (keeps trailing cells unshifted).
