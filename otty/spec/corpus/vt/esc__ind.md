<!-- source: https://docs.otty.sh/vt/esc/ind -->

# IND — Index

|  |  |
| --- | --- |
| Sequence | ESC D |
| Bytes | 0x1B 0x44 |
| Mnemonic | IND |
| Otty support | ✓ |

## Description

Moves the cursor down one row, scrolling the active region if the cursor is at the bottom. Equivalent in effect to [LF](/vt/c0/lf) but encoded as a two-byte ESC sequence — useful inside escape sequences where you don't want to use a raw control byte.

## Example
bash
```
printf 'a\eDb'      # 'a', index down, 'b' (b appears below a, same column)
```

## Related

- [LF](/vt/c0/lf), [NEL](/vt/esc/nel), [RI](/vt/esc/ri)
