<!-- source: https://docs.otty.sh/vt/esc/decrc -->

# DECRC — Restore Cursor

|  |  |
| --- | --- |
| Sequence | ESC 8 |
| Bytes | 0x1B 0x38 |
| Mnemonic | DECRC |
| Otty support | ✓ |

## Description

Restores the cursor state previously saved by [DECSC](/vt/esc/decsc). If no DECSC has been issued for the active screen, DECRC moves the cursor to home (`1, 1`) with default attributes.

Restores: cursor position, SGR attributes, character set selection, origin mode, auto-wrap mode.

## Example
bash
```
printf '\e7'           # save
printf '\e[31mred\e[m\n'
printf '\e8'           # restore cursor and SGR
```

## Related

- [DECSC](/vt/esc/decsc)
