<!-- source: https://docs.otty.sh/vt/csi/decscusr -->

# DECSCUSR — Set Cursor Style [​](#decscusr-—-set-cursor-style)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> SP q |
| Bytes | 0x1B 0x5B <n> 0x20 0x71 |
| Mnemonic | DECSCUSR |
| Otty support | ✓ |

## Description [​](#description)

Sets the cursor shape and blink state. Note the literal `SP` (space, `0x20`) between the parameter and the final `q`.
````````````````
| <n> | Style |
| --- | --- |
| 0, 1 | Block, blinking |
| 2 | Block, steady |
| 3 | Underline, blinking |
| 4 | Underline, steady |
| 5 | Beam, blinking |
| 6 | Beam, steady |

Programs that set the cursor style typically restore it (or send `0`) before exiting. The user default lives in the [config](/reference/configuration) (`cursor-style` and `cursor-style-blink`).

The shape always takes effect. The blink bit is honored only when the [`cursor-style-blink`](/reference/configuration) policy is one of the `default_*` values; the `always_*` values pin the blink and ignore it (as well as [DEC mode 12](/vt/concepts/cursor)). A program can query the current style with DECRQSS (`DCS $ q SP q ST`).

## Example [​](#example)
bash
```
printf '\e[5 q'      # beam, blinking
printf '\e[2 q'      # block, steady
printf '\e[0 q'      # back to user default
```

Try it

`otty features try cursor-shape` — cycles through every DECSCUSR shape with 700 ms holds so you can eyeball each one.

## Related [​](#related)

- [Cursor](/vt/concepts/cursor)
