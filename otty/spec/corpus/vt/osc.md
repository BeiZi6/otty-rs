<!-- source: https://docs.otty.sh/vt/osc -->

# OSC Sequences

Operating System Command — `ESC ]` followed by a numeric command, semicolon-separated arguments, and a string terminator. Used for cross-cutting concerns like window title, hyperlinks, clipboard, and shell integration.

## Structure

```
ESC ] <Ps> ; <Pt> ST
```

- **`Ps`**: numeric command (e.g. `0` set title, `8` hyperlink, `133` prompt mark)
- **`Pt`**: payload — free-form text, format depends on `Ps`
- **`ST`**: string terminator — `ESC \` (preferred) or `BEL` (legacy; both accepted)

For example:

- `OSC 0 ; My Window \e\\` — sets window title to "My Window"
- `OSC 8 ; ; https://example.com \e\\Visit\e]8;;\e\\` — wraps "Visit" in a hyperlink

## Detail pages

- [OSC 0 / 2 — Set Title](/vt/osc/osc-0-2)
- [OSC 4 — Palette Set / Query](/vt/osc/osc-4)
- [OSC 5 — Special Colors](/vt/osc/osc-5)
- [OSC 7 — Working Directory](/vt/osc/osc-7)
- [OSC 8 — Hyperlinks](/vt/osc/osc-8)
- [OSC 9 — Notification (iTerm)](/vt/osc/osc-9)
- [OSC 9;4 — Progress / Task State (ConEmu)](/vt/osc/osc-9-4)
- [OSC 10-19 — Dynamic Colors](/vt/osc/osc-10-19)
- [OSC 22 — Mouse Cursor Shape](/vt/osc/osc-22)
- [OSC 52 — Clipboard](/vt/osc/osc-52)
- [OSC 88 — Terminal Resume Protocol](/vt/osc/osc-88) — *Proposal*
- [OSC 99 — Kitty Notification Protocol](/vt/osc/osc-99)
- [OSC 104 / 105 / 110-119 — Color Reset](/vt/osc/osc-104-119)
- [OSC 133 — Shell Integration](/vt/osc/osc-133)
- [OSC 777 — Notification (urxvt)](/vt/osc/osc-777)
- [OSC 1337 — Inline Image](/vt/osc/osc-1337)
- See also [CSI #P/Q/R — XTPUSHCOLORS](/vt/csi/xt-colors)

Other OSCs are listed in [Sequence Reference](/vt/sequence-reference) → "OSC".

## See also

- [Shell Integration](/terminal-features/shell-integration) — uses OSC 7, OSC 133.
- [Notifications](/terminal-features/notifications) — uses OSC 9, OSC 99, OSC 777.
- [Cursor and Mouse](/terminal-features/cursor-and-mouse) — OSC 22 mouse cursor shape.
- [Images](/terminal-features/images) — uses OSC 1337.
