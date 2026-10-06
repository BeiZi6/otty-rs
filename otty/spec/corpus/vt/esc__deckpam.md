<!-- source: https://docs.otty.sh/vt/esc/deckpam -->

# DECKPAM — Keypad Application Mode

|  |  |
| --- | --- |
| Sequence | ESC = |
| Bytes | 0x1B 0x3D |
| Mnemonic | DECKPAM |
| Otty support | ✓ |

## Description

Switches the numeric keypad to **application mode**. In application mode the keypad keys send escape sequences instead of their numeric values, so programs can bind them as function keys.

Pair with [DECKPNM](/vt/esc/deckpnm) (`ESC >`) to switch back to numeric mode.

Most TUIs (`vim`, `htop`) enter application mode on start and exit it on quit; you rarely need to issue these manually.

## Example
bash
```
printf '\e='           # enter application keypad mode
# … now keypad '5' sends \eOu instead of '5'
printf '\e>'           # back to numeric
```

## Related

- [DECKPNM](/vt/esc/deckpnm)
