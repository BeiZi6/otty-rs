<!-- source: https://docs.otty.sh/vt/concepts/screen -->

# Screen [​](#screen)

The "screen" is the rectangular viewport visible inside a pane. Above it is **scrollback**; below it (on resize) gets reflowed.

## Main vs alternate screen [​](#main-vs-alternate-screen)

A terminal has two grids:

- **Main screen** — the normal scrolling buffer. Output appends; old lines move into scrollback.
- **Alternate screen** — a separate fixed-size grid with no scrollback. Used by full-screen TUIs (`vim`, `htop`, `less`).

Programs switch between them with DEC private modes `?47` / `?1047` / `?1049` — all three supported. They differ only in how they treat the cursor and the alternate buffer:

- `?47` — bare swap (legacy). No cursor save; the alternate buffer keeps whatever it held.
- `?1047` — swap, and **clear** the alternate buffer when switching back to the main screen.
- `?1049` — **save** the cursor and clear the alternate buffer on entry, **restore** the cursor on exit. The most common today (what `vim` / `htop` issue).

When you `:q` out of `vim`, the screen contents you see are the **main** screen, restored — the alternate screen and its contents are discarded.

## Scrollback [​](#scrollback)

The main screen retains lines that scrolled off the top, up to a configurable cap. Default `scrollback-rows = 10000` per pane.

- TUIs running on the alternate screen don't write to scrollback.
- `printf '\e[3J'` (ED with parameter 3) **clears scrollback** in addition to the visible screen. `clear` (some shells) does this.

## Scrolling regions [​](#scrolling-regions)

`CSI <top>;<bot> r` (DECSTBM) sets a vertical scrolling region. Line-feed at the bottom row scrolls only this region. Used by status-bar applications. Reset with `CSI r`.

`CSI <l>;<r> s` (DECSLRM) sets a horizontal region — supported but rare.

## Resize [​](#resize)

When the pane resizes, the main screen **reflows**: lines longer than the new width are wrapped, shorter wrapped lines may join. The alternate screen never reflows — the program redraws.

## See also [​](#see-also)

- [CSI erase commands](/vt/csi/) — ED, EL
- [Cursor](/vt/concepts/cursor) — interacts with scrolling regions via origin mode.
