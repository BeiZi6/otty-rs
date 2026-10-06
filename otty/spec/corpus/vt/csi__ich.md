<!-- source: https://docs.otty.sh/vt/csi/ich -->

# ICH — Insert Characters

|  |  |
| --- | --- |
| Sequence | CSI <n> @ |
| Bytes | 0x1B 0x5B <n> 0x40 |
| Mnemonic | ICH |
| Otty support | ✓ |

## Description

Inserts `<n>` blank cells at the cursor position, shifting existing content to the right. Cells that move past the right margin are lost.

Default `<n> = 1`. Cursor position itself is unchanged.

## Example
bash
```
printf 'abcdef\e[H'      # write text, move home
printf '\e[3@'           # insert 3 blanks at col 1
# row now reads: "   abcdef" (last "f" was pushed off-screen if width=8)
```

## Related

- [DCH](/vt/csi/dch) — delete characters (inverse).
