<!-- source: https://docs.otty.sh/vt/c0/can -->

# CAN — Cancel

|  |  |
| --- | --- |
| Byte | 0x18 |
| Mnemonic | CAN |
| Otty support | ✓ |

## Description

Aborts any control sequence currently being parsed. After CAN the parser returns to the ground state — partially-received CSI / OSC / DCS payloads are silently discarded.

Useful for clients that detected an upstream desync and want to be sure the terminal is in a clean parsing state before sending more.

## Example
bash
```
printf '\e[31\x18m'      # an SGR was started but cancelled mid-way;
                          # the 'm' that follows is rendered as text.
```

## Related

- [SUB](/vt/c0/sub) — identical effect.
- [Control Sequences](/vt/concepts/control-sequences) — how the parser handles aborts.
