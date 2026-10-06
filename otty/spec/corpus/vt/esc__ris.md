<!-- source: https://docs.otty.sh/vt/esc/ris -->

# RIS — Full Reset

|  |  |
| --- | --- |
| Sequence | ESC c |
| Bytes | 0x1B 0x63 |
| Mnemonic | RIS |
| Otty support | ✓ |

## Description

Resets the terminal to its power-on state:

- Switches to the main screen, clears it, **clears scrollback**.
- Resets cursor to home, default SGR, default character sets.
- Restores default tab stops (every 8 columns).
- Disables every DEC private mode set via [DECSET](/vt/csi/decset).
- Clears any saved cursor state.

Use sparingly — RIS is a heavy hammer. Most programs need [DECSTR](/vt/csi/decstr) (soft reset) instead, which preserves scrollback.

## Example
bash
```
printf '\ec'        # equivalent to `reset` on Unix, but more nuclear
```

## Related

- [DECSTR](/vt/csi/decstr) — soft reset (preferred for most cases).
