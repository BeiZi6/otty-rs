<!-- source: https://docs.otty.sh/vt/c0/esc -->

# ESC — Escape [​](#esc-—-escape)
****``********
|  |  |
| --- | --- |
| Byte | 0x1B |
| Mnemonic | ESC |
| Otty support | ✓ |

## Description [​](#description)

The single most important control byte. ESC introduces every multi-byte sequence in the terminal vocabulary — CSI, OSC, ESC-final, DCS, APC, and so on.

A bare ESC (followed by no recognised intermediate / final) resets the parser. In an OSC or DCS payload, ESC begins a string terminator (`ESC \`).

In shell readline and many editors, the ESC key is also used as a meta prefix — `Esc Esc` cancels, `Esc x` enters meta mode.

## Example [​](#example)
bash
```
printf '\e[31m red \e[0m\n'    # CSI SGR red, reset
printf '\e]0;My Title\e\\'     # OSC 0 set title, ESC \ terminator
```

## Related [​](#related)

- [Control Sequences](/vt/concepts/control-sequences) — full family overview.
- [CSI](/vt/csi/), [OSC](/vt/osc/), [ESC sequences](/vt/esc/)
