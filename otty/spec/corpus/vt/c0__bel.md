<!-- source: https://docs.otty.sh/vt/c0/bel -->

# BEL — Bell [​](#bel-—-bell)
****``********
|  |  |
| --- | --- |
| Byte | 0x07 |
| Mnemonic | BEL |
| Otty support | ✓ |

## Description [​](#description)

Rings the terminal bell. Modern terminals translate the byte into one of three behaviours depending on user preference:

- **Audible** — plays the system alert sound.
- **Visual** — briefly flashes the focused pane.
- **None** — silent.

In Otty, configure with `bell = visual | audio | both | none`. BEL is also accepted as a legacy [OSC string terminator](/vt/osc/) in place of `ESC \`.

## Example [​](#example)
bash
```
printf '\a'              # \a is the C escape for BEL
```

## Related [​](#related)

- [Notifications](/terminal-features/notifications) — bell config and desktop notifications.
- [OSC](/vt/osc/) — BEL also terminates an OSC payload.
