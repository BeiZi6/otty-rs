<!-- source: https://docs.otty.sh/vt/c0/ff -->

# FF — Form Feed [​](#ff-—-form-feed)
****``********
|  |  |
| --- | --- |
| Byte | 0x0C |
| Mnemonic | FF |
| Otty support | ✓ (treated as LF) |

## Description [​](#description)

Historically advanced a printer to the next page. On terminal emulators FF is treated as [LF](/vt/c0/lf) — Otty follows this convention. Some shells use `Ctrl-L` (which sends FF) as a clear-screen idiom; that's handled by the shell, not the terminal.

## Example [​](#example)
bash
```
printf 'page1\fpage2\n'   # \f is the C escape for FF
                           # behaves like LF
```

## Related [​](#related)

- [LF](/vt/c0/lf)
- [ED](/vt/csi/ed) — what `Ctrl-L` actually invokes in most shells.
