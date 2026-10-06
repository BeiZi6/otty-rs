<!-- source: https://docs.otty.sh/vt/osc/osc-22 -->

# OSC 22 — Mouse Cursor Shape

|  |  |
| --- | --- |
| Sequence | OSC 22 ; <shape> ST |
| Otty support | ✓ |

## Description

Asks the terminal to change the mouse pointer shape while the cursor is over the terminal pane. Introduced by xterm, extended and popularized by kitty; widely used by full-screen TUIs (yazi, btop, htop, mc, ranger) and mouse-driven REPLs to telegraph "this region is draggable / clickable / inert".

`<shape>` is a standard [CSS cursor name](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor). Otty recognizes the full set; shapes with no exact macOS equivalent fall back to a sensible neighbour (e.g. diagonal resize → horizontal resize, since macOS has no diagonal resize cursor). Passing `default` (or an empty value) hands control back to Otty's own cursor policy — I-beam over text, pointing hand over hyperlinks, crosshair with Option held.

## Supported shapes

The most commonly emitted shapes and the pointer Otty shows for each:

| Shape | Pointer shown | Typical usage |
| --- | --- | --- |
| default | (reset) | hand control back to Otty |
| pointer | pointing hand | clickable link / button |
| text | I-beam | editable text region |
| vertical-text | vertical I-beam | vertical CJK text |
| crosshair, cell | crosshair | precise grid selection |
| wait, progress | arrow | busy (macOS has no spinner cursor) |
| help, context-menu | contextual-menu arrow | hint / context affordance |
| move, all-scroll | closed hand | drag / pan |
| grab | open hand | draggable handle |
| grabbing | closed hand | actively dragging |
| no-drop, not-allowed | "operation not allowed" | drop forbidden |
| alias | drag-link arrow | alias / shortcut drop |
| copy | drag-copy arrow | copy-drop |
| n-resize, s-resize, ns-resize, row-resize | up/down resize | vertical edge |
| e-resize, w-resize, ew-resize, col-resize | left/right resize | horizontal edge |
| ne/sw/nw/se-resize, nesw-resize, nwse-resize | left/right resize | diagonal (best-effort) |
| zoom-in, zoom-out | arrow | zoom (no dedicated cursor) |

## Example
bash
```
# Switch to "pointer" while a TUI button is hovered, restore on leave:
printf '\e]22;pointer\e\\'
# … later …
printf '\e]22;default\e\\'

# Cycle through several shapes for a demo:
otty features try mouse-shape
```

## Behaviour notes

- The requested shape persists across PTY chunks and survives screen / alt-screen switches — only `default` (or an empty payload) resets it. Programs that take over the cursor should always restore it before exiting.
- When the program also subscribes to mouse reporting (DECSET 1000/1002/1003), Otty normally swaps in `arrow` so the surface feels GUI-like. An active OSC 22 shape **overrides** that fallback — programs that pick their own cursor get exactly what they asked for, matching kitty / Ghostty.
- `Cmd`-hover (open-link preview) always wins over OSC 22 because it expresses local user intent.

## Related

- [`otty features try mouse-shape`](#example) — built-in demo cycling the canonical shapes.
- [Cursor and Mouse](/terminal-features/cursor-and-mouse)
