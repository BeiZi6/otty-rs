<!-- source: https://docs.otty.sh/vt/c0/so -->

# SO — Shift Out [​](#so-—-shift-out)
****``********
|  |  |
| --- | --- |
| Byte | 0x0E |
| Mnemonic | SO |
| Otty support | ✓ |

## Description [​](#description)

Switches the active character set to **G1**. Combined with `ESC ( B` / `ESC ) 0` etc. to designate which character set G0 / G1 point at.

In practice this is used to enable the DEC special graphics charset (line-drawing characters) without VT100 escape sequences. Modern Unicode-aware programs use the Unicode box-drawing range directly instead — see [Box Drawing](/terminal-features/box-drawing).

## Example [​](#example)
bash
```
# Designate G1 as DEC Special Graphics, switch to it, draw a corner, switch back.
printf '\e)0\x0el\x0f'
# 0x0e = SO, 0x0f = SI, 'l' in DEC SG is "top-left corner"
```

## Related [​](#related)

- [SI](/vt/c0/si) — switches back to G0.
- [Box Drawing](/terminal-features/box-drawing)
