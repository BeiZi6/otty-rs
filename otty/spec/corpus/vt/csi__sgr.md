<!-- source: https://docs.otty.sh/vt/csi/sgr -->

# SGR — Select Graphic Rendition

|  |  |
| --- | --- |
| Sequence | CSI <p1> ; <p2> ; … m |
| Bytes | 0x1B 0x5B <params> 0x6D |
| Category | CSI |
| Mnemonic | SGR |
| Otty support | ✓ Implemented (full standard + extended underlines + RGB) |

## Description

Sets graphic rendition for subsequent characters — foreground / background color, weight, italic, underline, strike-through, etc. Parameters are 1+ numeric codes separated by `;` (or `:` for sub-parameters); multiple codes apply in order.

`CSI m` (no parameters) and `CSI 0 m` both **reset** all attributes.

## Common parameters

| Param | Effect |
| --- | --- |
| 0 | Reset |
| 1 | Bold |
| 2 | Dim |
| 3 | Italic |
| 4 | Underline |
| 5 | Blink |
| 7 | Reverse video |
| 8 | Conceal |
| 9 | Strike-through |
| 21 | Double underline |
| 22 | Reset bold/dim |
| 23 | Reset italic |
| 24 | Reset underline |
| 30–37 | Foreground ANSI 0-7 |
| 38;5;<n> | Foreground 256-color |
| 38;2;<r>;<g>;<b> | Foreground 24-bit |
| 39 | Default foreground |
| 40–47 | Background ANSI 0-7 |
| 48;5;<n> | Background 256-color |
| 48;2;<r>;<g>;<b> | Background 24-bit |
| 49 | Default background |
| 90–97 | Foreground bright 0-7 |
| 100–107 | Background bright 0-7 |

## Extended underlines

Using `:` sub-parameters (the kitty / iTerm2 / WezTerm convention). All five styles render in Otty:

| Sub-param | Style | Otty rendering |
| --- | --- | --- |
| 4:0 | None | clears underline |
| 4:1 | Single | 2 px underline (also via plain \e[4m) |
| 4:2 | Double | two stacked 2 px lines (use 4:2, not \e[21m — under ECMA-48, 21 resets bold) |
| 4:3 | Curly (undercurl) | square-wave wavy line, fg or SGR 58 colored |
| 4:4 | Dotted | repeating dot pattern |
| 4:5 | Dashed | repeating short-dash pattern |
| 58:5:<n> or 58:2::<r>:<g>:<b> | Underline color (independent of text color) | applied to single / double / curly / dotted / dashed |
| 59 | Reset underline color | back to text color |

Note: kitty's writers usually emit the colon form `58:2::R:G:B` (the empty colorspace slot before the RGB triple); the semicolon form `58;2;R;G;B` is also accepted.

## Example
bash
```
# Bold red text
printf '\e[1;31mERROR\e[0m\n'

# 24-bit color
printf '\e[38;2;127;90;240mpurple\e[0m\n'

# Curly red underline beneath default-colored text — the SGR 58 form
# nvim's LSP uses for error squiggles
printf '\e[4:3;58:2::255:100:100mtypo\e[0m\n'

# Dashed green underline (warnings)
printf '\e[4:5;58:2::120:255:120mTODO\e[0m\n'

# Try every style at once:
otty features try styles
```

Try it

- `otty features try styles` — bold / italic / underline / strike / reverse / blink sampler.
- `otty features try true-color` — 24-bit gradient strip across bg and fg.
- `otty features try text-style` — ASCII × CJK trait stress (catches font-cascade slips).
- `otty features try font-blending` — same line on dark / medium / light bg for comparing `font-blending` modes.
- `otty features try change-color` — interactive OSC 10/11/12/17/19 + XTPUSHCOLORS picker.

## Notes

- Parameter `1` (bold) renders in the **bright palette** when `bold-is-bright = on` (config). Off by default.
- Otty caps SGR parsing at 256 parameters per sequence; very long chains beyond that are truncated.
- Some terminals interpret `1` as a "bright" hint instead of bold weight; Otty applies real weight from the font.

## Related

- [Unicode and Text Styles](/terminal-features/unicode-and-text-styles) — how SGR maps to glyph drawing.
- [Themes](/customization/themes) — palette that ANSI codes (30-37 / 40-47 etc.) reference.
