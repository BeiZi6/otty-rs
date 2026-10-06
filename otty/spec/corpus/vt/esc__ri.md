<!-- source: https://docs.otty.sh/vt/esc/ri -->

# RI — Reverse Index

|  |  |
| --- | --- |
| Sequence | ESC M |
| Bytes | 0x1B 0x4D |
| Mnemonic | RI |
| Otty support | ✓ |

## Description

Moves the cursor up one row, scrolling the active region down if the cursor is at the top. The opposite of [IND](/vt/esc/ind).

## Example
bash
```
printf 'a\n\eM'      # write 'a', LF down, RI back up
```

## Related

- [IND](/vt/esc/ind)
