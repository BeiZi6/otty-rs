<!-- source: https://docs.otty.sh/vt/csi/decstr -->

# DECSTR — Soft Terminal Reset [​](#decstr-—-soft-terminal-reset)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI ! p |
| Bytes | 0x1B 0x5B 0x21 0x70 |
| Mnemonic | DECSTR |
| Otty support | ✓ |

## Description [​](#description)

Resets the terminal **state** without wiping the screen. Restores:

- SGR attributes to defaults
- Cursor visibility, cursor style
- DECOM, DECAWM, application keypad mode
- Saved cursor (DECSC) is cleared
- Scrolling region restored to full screen
- All DEC private modes restored to their power-on defaults

What it does **not** touch: visible screen contents, scrollback, tab stops, character set assignments.

Use DECSTR after a misbehaving program leaves the terminal in a weird state. Preferred over [RIS](/vt/esc/ris) (full reset) in most situations.

## Example [​](#example)
bash
```
printf '\e[!p'        # soft reset
```

## Related [​](#related)

- [RIS](/vt/esc/ris)
