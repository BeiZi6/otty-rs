<!-- source: https://docs.otty.sh/vt/concepts/colors -->

# Colors [​](#colors)

Otty supports four color depths, layered: 24-bit truecolor, 256-color indexed, ANSI 16, and the default (no SGR).

## The 16-color palette [​](#the-16-color-palette)

The first 16 entries of the palette are the classic ANSI colors plus their "bright" variants:

| Index | Name | Index | Name |
| --- | --- | --- | --- |
| 0 | black | 8 | bright black |
| 1 | red | 9 | bright red |
| 2 | green | 10 | bright green |
| 3 | yellow | 11 | bright yellow |
| 4 | blue | 12 | bright blue |
| 5 | magenta | 13 | bright magenta |
| 6 | cyan | 14 | bright cyan |
| 7 | white | 15 | bright white |

Set via SGR `30`–`37` / `40`–`47` (regular) and `90`–`97` / `100`–`107` (bright). The actual RGB values come from the active [Theme](/customization/themes).

## The 256-color palette [​](#the-256-color-palette)

Indices `0`–`15`: same as above. Indices `16`–`231`: a 6×6×6 RGB cube. Indices `232`–`255`: 24 shades of grey.

Set with `SGR 38;5;<n>` (foreground) or `48;5;<n>` (background). See [SGR](/vt/csi/sgr).

## 24-bit truecolor [​](#_24-bit-truecolor)

`SGR 38;2;<r>;<g>;<b>` (foreground) or `48;2;<r>;<g>;<b>` (background). Each channel `0–255`.

Otty advertises truecolor via `COLORTERM=truecolor` in the environment. Programs that gate on this env var (Neovim, modern shells) light up automatically.

## Default colors [​](#default-colors)

`SGR 39` resets foreground to the theme default; `SGR 49` resets background. These are not "black" or "white" — they're whatever the theme defines as default.

## Bold-is-bright [​](#bold-is-bright)

A legacy convention: rendering `bold` text in the brightness palette. Off in Otty by default. Toggle with `bold-is-bright = on` in [Themes](/customization/themes).

## Color reporting (OSC 4 / 10 / 11) [​](#color-reporting-osc-4-10-11)

Programs can query and modify palette entries at runtime:

- `OSC 4 ; <index> ; <spec>` — set / query palette entry
- `OSC 10 ; <spec>` — set / query default foreground
- `OSC 11 ; <spec>` — set / query default background

A query is `<spec> = "?"`; the terminal replies with the current value.

## See also [​](#see-also)

- [SGR](/vt/csi/sgr) — the command that uses these.
- [Theme Format](/reference/theme-format) — how to define your own palette.
