<!-- source: https://docs.otty.sh/vt/c0/lf -->

# LF — Line Feed [​](#lf-—-line-feed)
****``********
|  |  |
| --- | --- |
| Byte | 0x0A |
| Mnemonic | LF |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor down one row. The column does **not** change — to start a new line in the leftmost column you need `CR LF` (or [NEL](/vt/esc/nel)).

If the cursor is at the bottom of the active region, LF scrolls the region up by one line.

When LNM (line-feed/new-line mode, `CSI 20 h`) is set, LF behaves like NEL (also moves to column 1). LNM is off by default; almost all modern programs assume CR-then-LF for newlines.

## Example [​](#example)
bash
```
printf 'line1\nline2'    # \n is the C escape for LF
                          # output: line1 \n line2 — note "line2" starts under
                          # the last column of "line1" if no CR is sent
```

## Related [​](#related)

- [CR](/vt/c0/cr) — pair with LF for "newline".
- [NEL](/vt/esc/nel) — combined CR + LF in one byte sequence.
