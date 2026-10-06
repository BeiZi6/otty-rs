<!-- source: https://docs.otty.sh/vt/csi/dsr -->

# DSR — Device Status Report [​](#dsr-—-device-status-report)
****``****``************``
|  |  |
| --- | --- |
| Sequence | CSI <Ps> n |
| Bytes | 0x1B 0x5B <Ps> 0x6E |
| Category | CSI |
| Mnemonic | DSR |
| Otty support | ✓ Implemented (Ps = 5, 6) |

## Description [​](#description)

A host program asks the terminal about its state and the terminal answers by writing a reply back up the PTY. Otty answers two queries immediately:
````````[]````****
| Query | Reply | Meaning |
| --- | --- | --- |
| CSI 5 n | CSI 0 n | Terminal is ready / OK (no malfunction). |
| CSI 6 n | CSI <row> ; <col> R | Cursor Position Report (CPR). row / col are 1-based. |

The `CSI 6 n` form is the one most programs care about: it's how a shell or TUI discovers where the cursor actually landed after printing text — for example to detect whether a prompt left a trailing partial line, or to measure the width of already-emitted output.

## Example [​](#example)
bash
```
# Ask for the cursor position. The reply (e.g. `\e[12;1R`) is written to the
# terminal's input, so in a raw-mode program you'd read it back from stdin.
printf '\e[6n'

# Ask whether the terminal is OK — reply is `\e[0n`.
printf '\e[5n'
```

Because the reply arrives on the same channel as keyboard input, capturing it requires the program to put the terminal in raw mode and read stdin; you won't see the response if you just run the `printf` at an interactive shell prompt (the shell consumes it).

## Not supported [​](#not-supported)

- **DECXCPR** (`CSI ? 6 n`) — the DEC private variant that adds a page number, replying `CSI ? <row> ; <col> ; <page> R`. Otty implements only the standard `CSI 6 n` form above. Single-page terminals essentially never need DECXCPR.
- Other `Ps` values (e.g. printer / keyboard / locator status) are not answered.

## Related [​](#related)

- [CUP — Cursor Position](/vt/csi/cup) — moves the cursor to the position DSR reports.
- `ESC 7` / `ESC 8` — save / restore the full cursor state.
- [Terminal identification](/terminal-features/term-value) — DA / XTVERSION and other reports the terminal sends about itself.

## Notes [​](#notes)

- The reply is sent as a PTY write, ordered with respect to other output the terminal generates.
- Positions are reported relative to the viewport (or the scrolling region when origin mode `DECOM` / `?6` is active), matching how [CUP](/vt/csi/cup) interprets them.
