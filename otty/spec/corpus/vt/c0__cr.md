<!-- source: https://docs.otty.sh/vt/c0/cr -->

# CR — Carriage Return [​](#cr-—-carriage-return)
****``********
|  |  |
| --- | --- |
| Byte | 0x0D |
| Mnemonic | CR |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor to column 1 of the current row. **Does not** change the row.

A typical "newline" is `CR LF` (`\r\n`) — CR to the leftmost column, LF down one row. On Unix-line-ending files, `\n` (LF) is enough only because LNM mode is off and shells issue an implicit CR via NEL-style logic; raw `printf '\n'` mid-line moves down without resetting the column.

## Example [​](#example)
bash
```
printf 'loading\r'             # cursor returns to col 1 on the same line
printf 'loading\rdone   \n'    # overwrites with "done   "
```

CR is also handy for progress bars: emit `\r` then redraw the line.

## Related [​](#related)

- [LF](/vt/c0/lf)
- [NEL](/vt/esc/nel) — explicit "next line".
