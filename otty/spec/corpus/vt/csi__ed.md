<!-- source: https://docs.otty.sh/vt/csi/ed -->

# ED — Erase in Display [​](#ed-—-erase-in-display)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> J |
| Bytes | 0x1B 0x5B <n> 0x4A |
| Mnemonic | ED |
| Otty support | ✓ |

## Description [​](#description)

Erases part of the display. Default `<n> = 0`.
``````````****``
| <n> | Effect |
| --- | --- |
| 0 | Erase from cursor to end of screen |
| 1 | Erase from start of screen to cursor |
| 2 | Erase the whole screen |
| 3 | Erase the whole screen and scrollback |
| 22 | Push the entire viewport into scrollback, then erase (kitty extension) |

`CSI 3 J` is an xterm extension. `clear` on Unix typically emits `CSI 2 J` followed by [CUP](/vt/csi/cup) home; modern `clear` increasingly emits `CSI 3 J` to clear scrollback.

`CSI 22 J` is a kitty extension supported by Otty, kitty, WezTerm, Ghostty, and Contour. Unlike `CSI 2 J`, which erases the visible viewport in place and drops its content, `CSI 22 J` first scrolls the current viewport into scrollback so the user can scroll back to see what was on screen. Apps targeting a "Ctrl-L that preserves history" UX should emit `\e[22J\e[H` instead of `\e[2J\e[H`. Terminals that don't implement it silently drop the bytes, so emitting it is safe across the ecosystem — supporting terminals get scrollback preservation, others see the same result as if no clear had been sent.

Erased cells become the default background; cursor position is unchanged (apart from the implicit scroll for `n=22`).

## DECSED — Selective Erase in Display [​](#decsed-—-selective-erase-in-display)

`CSI ? <n> J` (note the `?` private marker) is the **selective** variant. It erases only cells that are **not** protected by [DECSCA](/vt/csi/decsca), leaving protected cells untouched. `<n>` takes the same `0` / `1` / `2` meanings as plain ED.

Unlike `CSI 2 J`, which on the main screen scrolls the viewport into scrollback, DECSED always erases **in place** and never touches scrollback.
bash
```
printf '\e[1"q'        # DECSCA: protect what follows
printf 'keep me'
printf '\e[0"q'        # DECSCA: stop protecting
printf ' erase me'
printf '\e[?2J'        # DECSED: 'erase me' clears, 'keep me' survives
```

Otty implements DECSED; some terminals (e.g. Alacritty) don't and ignore the sequence. See [Terminal Comparison](/vt/terminal-comparison).

## Example [​](#example)
bash
```
printf '\e[2J\e[H'     # clear screen and home the cursor (legacy `clear`)
printf '\e[3J\e[2J\e[H' # also drop scrollback
printf '\e[22J\e[H'    # push viewport into scrollback first, then clear (kitty extension)
```

Demo via the CLI:
bash
```
otty features try ed-scroll-clear
```

## Related [​](#related)

- [EL](/vt/csi/el) — erase one line.
- [DECSCA](/vt/csi/decsca) — mark cells protected from selective erase.
