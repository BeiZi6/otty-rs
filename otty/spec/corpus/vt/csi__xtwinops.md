<!-- source: https://docs.otty.sh/vt/csi/xtwinops -->

# XTWINOPS — Window Manipulation & Size Reports

|  |  |
| --- | --- |
| Sequence | CSI <Ps> ; <Ps> ; <Ps> t |
| Bytes | 0x1B 0x5B <Ps…> 0x74 |
| Mnemonic | XTWINOPS |
| Otty support | ◐ — read-only size reports only |

## Description

xterm's window-manipulation family. The first parameter `Ps` selects an operation: many of them *act* on the window (deiconify, move, resize, raise/lower, maximize), and a few *report* a dimension back over the PTY.

Otty deliberately implements **only the read-only size reports**, and only a safe subset of those. The window-*acting* operations (move / resize / raise / lower / iconify) are **ignored** — a remote program should not be able to move or resize your window, and on macOS those map poorly to the window server anyway.

| Ps | Operation | Otty |
| --- | --- | --- |
| 14 | Report text-area size in pixels → replies CSI 4 ; height ; width t | ✓ answered |
| 16 | Report cell size in pixels → replies CSI 6 ; height ; width t | ✓ answered |
| 18 | Report text-area size in characters → would reply CSI 8 ; rows ; cols t | ✗ not answered (by design) |
| 1–9, 10, 11, 13, 19, 20–24 | deiconify / move / resize / raise / lower / maximize / fullscreen / report position / title-stack | ✗ ignored |

### Why `CSI 18 t` is intentionally not answered

The character-size report (`Ps = 18`) is the one window op that is both **redundant and harmful**, so Otty drops it on purpose — matching xterm's own default (`allowWindowOps` is off by default for exactly this reason):

- **Redundant** — a program already learns the size in *characters* from the kernel (`TIOCGWINSZ`), the `$LINES` / `$COLUMNS` environment, and `SIGWINCH`. Nothing needs `CSI 18 t`.
- **Harmful through a multiplexer** — the reply `CSI 8 ; rows ; cols t` is delivered to the application's **stdin**. When a layer like **tmux** or **ssh** sits in between and doesn't consume it, the reply leaks down to the inner shell and is echoed as literal garbage at the prompt — e.g. `^[[8;34;126t` — and re-appears on every resize.

The **pixel** reports (`14` / `16`) are kept because there is no other portable way to get pixel geometry, and terminal **image protocols** (Sixel, the [kitty graphics protocol](/vt/external-protocols), [OSC 1337 inline images](/vt/osc/osc-1337)) genuinely need cell pixel size to scale images. Otty only answers them once a real cell size is known.

## Example
bash
```
# Ask for cell size in pixels — reply lands on the program's stdin
printf '\e[16t'      # → e.g. \e[6;34;15t  (cell 34px tall × 15px wide)

# Ask for text-area size in pixels
printf '\e[14t'      # → e.g. \e[4;1156;1890t

# Character size — Otty stays silent (use the shell instead):
printf '\e[18t'      # → (no reply)
echo "$LINES x $COLUMNS"
```

Reading the reply

Because the answer arrives on stdin, you can't see it with a bare `printf`. Tools that need it (chafa, timg, viu, image.nvim, …) put the terminal in raw mode, send the query, and read the `CSI 4/6 …t` answer directly.

## Notes

- Replies use the xterm parameter order `height ; width` (rows before columns; pixel-height before pixel-width).
- The pixel reports are gated on a known cell size: a program that queries before Otty's first layout gets no answer (same as xterm before the first resize).
- If you ever see `^[[8;..t` (or `;rows;colst`) text at your shell prompt inside tmux, it means an *older* build was still answering `CSI 18 t`; current builds do not.

## Related

- [DECSET / DECRST](/vt/csi/decset) — DEC private modes (mouse, alt-screen, bracketed paste).
- [OSC 1337](/vt/osc/osc-1337) — inline images, which rely on the pixel size reports.
- [External protocols](/vt/external-protocols) — Sixel / kitty graphics.
- [Terminal Comparison](/vt/terminal-comparison) — how Otty's window-op support lines up with other terminals.
