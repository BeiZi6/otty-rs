<!-- source: https://docs.otty.sh/vt/esc/deckpnm -->

# DECKPNM — Keypad Numeric Mode

|  |  |
| --- | --- |
| Sequence | ESC > |
| Bytes | 0x1B 0x3E |
| Mnemonic | DECKPNM |
| Otty support | ✓ |

## Description

Switches the numeric keypad to **numeric mode** — keypad keys send their literal digits / operators. This is the default for normal shell sessions.

## Example
bash
```
printf '\e>'      # numeric mode (default)
```

## Related

- [DECKPAM](/vt/esc/deckpam)
