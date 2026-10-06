<!-- source: https://docs.otty.sh/vt/csi/el -->

# EL — Erase in Line [​](#el-—-erase-in-line)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> K |
| Bytes | 0x1B 0x5B <n> 0x4B |
| Mnemonic | EL |
| Otty support | ✓ |

## Description [​](#description)

Erases part of the current line. Default `<n> = 0`.
````````
| <n> | Effect |
| --- | --- |
| 0 | Erase from cursor to end of line |
| 1 | Erase from start of line to cursor |
| 2 | Erase the whole line |

Cursor position is unchanged. Useful for redrawing a status line or progress bar without scrolling.

## DECSEL — Selective Erase in Line [​](#decsel-—-selective-erase-in-line)

`CSI ? <n> K` (note the `?` private marker) is the **selective** variant: it erases only cells **not** protected by [DECSCA](/vt/csi/decsca), with the same `0` / `1` / `2` meanings as plain EL. Protected cells on the line are left in place.
bash
```
printf '\e[1"q'; printf 'KEEP'; printf '\e[0"q'; printf ' wipe'
printf '\r\e[?2K'     # whole line selectively erased — 'KEEP' stays, ' wipe' clears
```

## Example [​](#example)
bash
```
printf '\r\e[K'      # CR to start of line, erase rest — typical "redraw line" idiom
printf 'progress: %d%%\r\e[K' 42
```

## Related [​](#related)

- [ED](/vt/csi/ed)
- [DECSCA](/vt/csi/decsca) — mark cells protected from selective erase.
- [CR](/vt/c0/cr)
