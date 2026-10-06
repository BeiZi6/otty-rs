<!-- source: https://docs.otty.sh/vt/c0/vt -->

# VT — Vertical Tab [​](#vt-—-vertical-tab)
****``********
|  |  |
| --- | --- |
| Byte | 0x0B |
| Mnemonic | VT |
| Otty support | ✓ (treated as LF) |

## Description [​](#description)

Historically advanced the cursor to the next vertical tab stop. Vertical tab stops are essentially never used on modern terminals — Otty (like every modern emulator) treats VT as a synonym for [LF](/vt/c0/lf).

## Example [​](#example)
bash
```
printf 'one\vtwo\n'      # \v is the C escape for VT
                          # behaves like LF: "two" appears on the next row
```

## Related [​](#related)

- [LF](/vt/c0/lf)
- [FF](/vt/c0/ff)
