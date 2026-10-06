<!-- source: https://docs.otty.sh/vt/csi/decrst -->

# DECRST — Reset DEC Private Mode [​](#decrst-—-reset-dec-private-mode)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI ? <Pn>;<Pn>… l |
| Bytes | 0x1B 0x5B 0x3F <Pn> 0x6C |
| Mnemonic | DECRST |
| Otty support | ✓ |

## Description [​](#description)

Disables one or more DEC private modes. Same mode numbers as [DECSET](/vt/csi/decset).

## Example [​](#example)
bash
```
printf '\e[?25l'        # hide cursor
printf '\e[?1049l'      # leave alternate screen, restore cursor (what vim issues on exit)
printf '\e[?1047l'      # leave alternate screen, clearing it first (legacy)
```

## Related [​](#related)

- [DECSET](/vt/csi/decset) — set the same modes.
