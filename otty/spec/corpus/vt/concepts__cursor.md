<!-- source: https://docs.otty.sh/vt/concepts/cursor -->

# Cursor [​](#cursor)

The cursor is the active write position. Programs move it explicitly; characters output advance it implicitly.

## Position [​](#position)

The cursor has a `(row, column)` position, **1-indexed**. `(1, 1)` is the top-left of the active region.

- Move absolute: [CUP](/vt/csi/cup) — `CSI <r>;<c> H`
- Move relative: [CUU / CUD / CUF / CUB](/vt/csi/) — `A`/`B`/`C`/`D`
- Save / restore: [DECSC](/vt/esc/decsc) / [DECRC](/vt/esc/decrc) — `ESC 7` / `ESC 8`

## Style [​](#style)

Selectable per program:

- `block` — the default, fills the cell.
- `underline` — a line at the cell baseline.
- `beam` — a thin vertical line on the left edge.

Set with [DECSCUSR](/vt/csi/decscusr) — `CSI <n> SP q`. User defaults via the [`cursor-style`](/customization/themes) config key.

## Blink [​](#blink)

Each style has a blinking and a steady variant. DECSCUSR encodes both into the same parameter.
````````````````
| <n> | Style |
| --- | --- |
| 0, 1 | Block, blinking |
| 2 | Block, steady |
| 3 | Underline, blinking |
| 4 | Underline, steady |
| 5 | Beam, blinking |
| 6 | Beam, steady |

Blinking can also be toggled independently of the shape with DEC private mode 12 (`CSI ? 12 h` = blink, `CSI ? 12 l` = steady).

Whether a program's blink request is honored depends on the **`cursor-style-blink`** policy: the `default_*` values defer to the program, while `always_*` pin the blink and ignore both DECSCUSR's blink bit and DEC mode 12. The cursor **shape** always follows the program. See [Cursor and Mouse](/terminal-features/cursor-and-mouse) and the [`cursor-style-blink`](/reference/configuration) reference.

A program can read the current cursor style back with DECRQSS: `DCS $ q SP q ST` → Otty replies `DCS 1 $ r <n> SP q ST` with the code from the table above.

## Visibility [​](#visibility)

Show / hide with DEC private modes `?25h` / `?25l`. Hidden cursor is common in TUIs that draw their own.

## Origin mode [​](#origin-mode)

`DECOM` (`?6`) makes `CUP` positions relative to the active scrolling region. Off by default.

## See also [​](#see-also)

- [Cursor and Mouse](/terminal-features/cursor-and-mouse)
- [CSI cursor commands](/vt/csi/)
